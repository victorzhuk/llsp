mod support;

use llsp::config::Layers;
use serde_json::{Value, json};
use support::Workspace;

fn labels(v: &Value) -> Vec<String> {
    v["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap().to_owned())
        .collect()
}

/// Position right after `needle`'s n-th occurrence.
fn after(ws: &Workspace, rel: &str, needle: &str, n: usize) -> Value {
    let mut p = ws.pos(rel, needle, n);
    let ch = p["position"]["character"].as_u64().unwrap() + needle.len() as u64;
    p["position"]["character"] = json!(ch);
    p
}

#[test]
fn completion_locals_before_workspace() {
    let ws = Workspace::new(&[
        ("a.lisp", "(defun helper () 1)"),
        ("b.lisp", "(let ((hello 1)) (he))"),
    ]);
    let mut c = ws.client(1);
    ws.open(&mut c, "b.lisp");
    let v = c
        .request_raw("textDocument/completion", after(&ws, "b.lisp", " (he", 0))
        .unwrap();
    let l = labels(&v);
    let hello = l.iter().position(|x| x == "hello").unwrap();
    let helper = l.iter().position(|x| x == "helper").unwrap();
    assert!(hello < helper, "{l:?}");
    let item = &v["items"][hello];
    assert_eq!(item["kind"], 6);
    assert_eq!(item["textEdit"]["range"]["start"]["character"], 18);
    assert_eq!(item["textEdit"]["range"]["end"]["character"], 20);
    c.shutdown();
}

#[test]
fn completion_skips_strings_and_comments() {
    let ws = Workspace::new(&[("a.lisp", "(f \"he\") ; he\n")]);
    let mut c = ws.client(0);
    ws.open(&mut c, "a.lisp");
    let v = c
        .request_raw("textDocument/completion", after(&ws, "a.lisp", "\"he", 0))
        .unwrap();
    assert!(v.is_null(), "{v}");
    let v = c
        .request_raw("textDocument/completion", after(&ws, "a.lisp", "; he", 0))
        .unwrap();
    assert!(v.is_null(), "{v}");
    c.shutdown();
}

#[test]
fn completion_builtins_toggle_and_cap() {
    let ws = Workspace::new(&[("a.lisp", "(ca)")]);
    let mut c = ws.client(0);
    ws.open(&mut c, "a.lisp");
    let v = c
        .request_raw("textDocument/completion", after(&ws, "a.lisp", "(ca", 0))
        .unwrap();
    assert!(labels(&v).contains(&"car".to_owned()));
    assert_eq!(labels(&v)[0], "car", "shorter prefix match first");
    c.shutdown();

    let layers = Layers {
        cli: toml::from_str("[completion]\nbuiltins = false").unwrap(),
        ..Layers::default()
    };
    let mut c = ws.client_with(layers, 0);
    ws.open(&mut c, "a.lisp");
    let v = c
        .request_raw("textDocument/completion", after(&ws, "a.lisp", "(ca", 0))
        .unwrap();
    assert!(!labels(&v).contains(&"car".to_owned()));
    c.shutdown();

    let layers = Layers {
        cli: toml::from_str("[completion]\nmax_items = 3").unwrap(),
        ..Layers::default()
    };
    let mut c = ws.client_with(layers, 0);
    ws.open(&mut c, "a.lisp");
    let v = c
        .request_raw("textDocument/completion", after(&ws, "a.lisp", "(ca", 0))
        .unwrap();
    assert_eq!(labels(&v).len(), 3);
    assert_eq!(v["isIncomplete"], true);
    c.shutdown();
}

#[test]
fn completion_qualified_alias() {
    let ws = Workspace::new(&[
        (
            "util.clj",
            "(ns app.util)\n(defn format-x [s] s)\n(defn other [] 1)",
        ),
        ("other.clj", "(ns app.other)\n(defn format-y [] 1)"),
        ("c.clj", "(ns c (:require [app.util :as u]))\n(u/fo)"),
    ]);
    let mut c = ws.client(5);
    ws.open(&mut c, "c.clj");
    let v = c
        .request_raw("textDocument/completion", after(&ws, "c.clj", "u/fo", 0))
        .unwrap();
    assert_eq!(labels(&v), ["format-x"]);
    let edit = &v["items"][0]["textEdit"]["range"];
    assert_eq!(edit["start"]["character"], 3);
    assert_eq!(edit["end"]["character"], 5);
    c.shutdown();
}

#[test]
fn signature_help_rest_and_arity() {
    let ws = Workspace::new(&[
        (
            "a.lisp",
            "(defun f (a &optional b &rest more) \"Doc f.\" a)\n(f 1 2 3 )",
        ),
        ("g.clj", "(defn g ([x] x) ([x y] y))\n(g 1 2)"),
    ]);
    let mut c = ws.client(2);
    ws.open(&mut c, "a.lisp");
    let v = c
        .request_raw(
            "textDocument/signatureHelp",
            after(&ws, "a.lisp", "(f 1 2 3", 0),
        )
        .unwrap();
    assert_eq!(v["signatures"][0]["label"], "(f a &optional b &rest more)");
    assert_eq!(v["activeParameter"], 4, "`3` maps to the rest param");
    assert_eq!(v["signatures"][0]["documentation"]["value"], "Doc f.");
    let v = c
        .request_raw(
            "textDocument/signatureHelp",
            after(&ws, "a.lisp", "(f 1", 0),
        )
        .unwrap();
    assert_eq!(v["activeParameter"], 0);
    let v = c
        .request_raw(
            "textDocument/signatureHelp",
            after(&ws, "a.lisp", "(f 1 ", 0),
        )
        .unwrap();
    assert_eq!(v["activeParameter"], 2, "skips &optional");

    ws.open(&mut c, "g.clj");
    let v = c
        .request_raw(
            "textDocument/signatureHelp",
            after(&ws, "g.clj", "(g 1 ", 0),
        )
        .unwrap();
    assert_eq!(v["activeSignature"], 1);
    assert_eq!(v["signatures"][1]["label"], "(g x y)");
    assert_eq!(v["activeParameter"], 1);
    c.shutdown();
}

#[test]
fn hover_kinds() {
    let ws = Workspace::new(&[(
        "a.lisp",
        "(in-package :geo)\n(defun area (w h) \"Area.\" (* w h))\n(let ((z 1)) (area z (car nil)))",
    )]);
    let mut c = ws.client(1);
    ws.open(&mut c, "a.lisp");
    let text = |v: Value| v["contents"]["value"].as_str().unwrap().to_owned();
    let h = text(
        c.request_raw("textDocument/hover", ws.pos("a.lisp", "area", 1))
            .unwrap(),
    );
    assert!(
        h.contains("(area w h)") && h.contains("Area.") && h.contains("`geo`"),
        "{h}"
    );
    let h = text(
        c.request_raw("textDocument/hover", ws.pos("a.lisp", "z (car", 0))
            .unwrap(),
    );
    assert!(h.contains("local binding"), "{h}");
    let h = text(
        c.request_raw("textDocument/hover", ws.pos("a.lisp", "car", 0))
            .unwrap(),
    );
    assert!(h.contains("builtin"), "{h}");
    let h = text(
        c.request_raw("textDocument/hover", ws.pos("a.lisp", "let", 0))
            .unwrap(),
    );
    assert!(h.contains("special form"), "{h}");
    let v = c
        .request_raw("textDocument/hover", ws.pos("a.lisp", "nil", 0))
        .unwrap();
    assert!(v.is_null());
    c.shutdown();
}
