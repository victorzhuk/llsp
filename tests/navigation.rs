mod support;

use llsp::config::Layers;
use serde_json::{Value, json};
use support::Workspace;

fn ranges(v: &Value) -> Vec<(String, u64, u64, u64)> {
    let mut out: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            let uri = l["uri"].as_str().unwrap();
            let name = uri.rsplit('/').next().unwrap().to_owned();
            let r = &l["range"];
            (
                name,
                r["start"]["line"].as_u64().unwrap(),
                r["start"]["character"].as_u64().unwrap(),
                r["end"]["character"].as_u64().unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn document_symbols_are_nested() {
    let ws = Workspace::new(&[(
        "a.lisp",
        "(defun a () (flet ((x ())) x))\n(defclass b () ())\n(progn (defvar *c* 1))",
    )]);
    let mut c = ws.client(3);
    ws.open(&mut c, "a.lisp");
    let v = c
        .request_raw(
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": ws.uri("a.lisp")}}),
        )
        .unwrap();
    let names: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["name"].clone(), s["kind"].clone()))
        .collect();
    assert_eq!(
        names,
        [
            (json!("a"), json!(12)),
            (json!("b"), json!(5)),
            (json!("*c*"), json!(13))
        ]
    );
    assert_eq!(v[0]["detail"], "()");
    assert_eq!(v[0]["selectionRange"]["start"]["character"], 7);
    c.shutdown();
}

#[test]
fn workspace_symbols_fuzzy_and_capped() {
    let ws = Workspace::new(&[(
        "p.lisp",
        "(defun make-point ()) (defun point-x ()) (defun mapcar-safe ()) (defun point-y ()) (defun point ())",
    )]);
    let mut c = ws.client(5);
    let v = c
        .request_raw("workspace/symbol", json!({"query": "mkp"}))
        .unwrap();
    let names: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["make-point"]);
    let v = c
        .request_raw("workspace/symbol", json!({"query": "point"}))
        .unwrap();
    let names: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["point", "point-x", "point-y", "make-point"]);
    c.shutdown();

    let layers = Layers {
        cli: toml::from_str("[workspace]\nmax_symbols = 2").unwrap(),
        ..Layers::default()
    };
    let mut c = ws.client_with(layers, 2);
    let v = c
        .request_raw("workspace/symbol", json!({"query": "p"}))
        .unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
    c.shutdown();
}

#[test]
fn definition_local_and_cross_file() {
    let ws = Workspace::new(&[
        ("a.lisp", "(defun helper (x) x)"),
        ("b.lisp", "(defun main ()\n  (let ((x 1)) x)\n  (helper 2))"),
        ("c.clj", "(defn only-clj [] 1)"),
        ("d.lisp", "(only-clj)"),
    ]);
    let mut c = ws.client(3);
    ws.open(&mut c, "b.lisp");
    let v = c
        .request_raw("textDocument/definition", ws.pos("b.lisp", "x)", 0))
        .unwrap();
    assert_eq!(ranges(&v), [("b.lisp".into(), 1, 9, 10)]);
    let v = c
        .request_raw("textDocument/definition", ws.pos("b.lisp", "helper", 0))
        .unwrap();
    assert_eq!(ranges(&v), [("a.lisp".into(), 0, 7, 13)]);
    ws.open(&mut c, "d.lisp");
    let v = c
        .request_raw("textDocument/definition", ws.pos("d.lisp", "only", 0))
        .unwrap();
    assert!(v.is_null(), "{v}");
    c.shutdown();
}

#[test]
fn definition_prefers_qualifier_namespace() {
    let ws = Workspace::new(&[
        ("a.clj", "(ns app.a)\n(defn run [] 1)"),
        ("b.clj", "(ns app.b)\n(defn run [] 2)"),
        ("c.clj", "(ns app.c (:require [app.b :as b]))\n(b/run)"),
    ]);
    let mut c = ws.client(5);
    ws.open(&mut c, "c.clj");
    let v = c
        .request_raw("textDocument/definition", ws.pos("c.clj", "run", 0))
        .unwrap();
    assert_eq!(ranges(&v), [("b.clj".into(), 1, 6, 9)]);
    c.shutdown();
}

#[test]
fn references_and_highlights() {
    let ws = Workspace::new(&[
        ("a.lisp", "(defun helper (x) x)\n(let ((x 1)) (+ x x))"),
        ("b.lisp", "(helper (helper 1))"),
    ]);
    let mut c = ws.client(1);
    ws.open(&mut c, "a.lisp");
    let mut p = ws.pos("a.lisp", "x 1", 0);
    p["context"] = json!({"includeDeclaration": false});
    let v = c.request_raw("textDocument/references", p.clone()).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
    p["context"] = json!({"includeDeclaration": true});
    assert_eq!(
        c.request_raw("textDocument/references", p)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let mut p = ws.pos("a.lisp", "helper", 0);
    p["context"] = json!({"includeDeclaration": true});
    let v = c.request_raw("textDocument/references", p.clone()).unwrap();
    assert_eq!(
        ranges(&v),
        [
            ("a.lisp".into(), 0, 7, 13),
            ("b.lisp".into(), 0, 1, 7),
            ("b.lisp".into(), 0, 9, 15)
        ]
    );
    p["context"] = json!({"includeDeclaration": false});
    assert_eq!(
        c.request_raw("textDocument/references", p)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let v = c
        .request_raw(
            "textDocument/documentHighlight",
            ws.pos("a.lisp", "x) x", 0),
        )
        .unwrap();
    let kinds: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["kind"].as_u64().unwrap())
        .collect();
    assert_eq!(kinds, [3, 2]);
    c.shutdown();
}

#[test]
fn rename_across_files_keeps_qualifier() {
    let ws = Workspace::new(&[
        ("a.lisp", "(in-package :app)\n(defun helper () 1)"),
        ("b.lisp", "(app::helper)\n(helper)"),
    ]);
    let mut c = ws.client(1);
    ws.open(&mut c, "b.lisp");
    let v = c
        .request_raw("textDocument/prepareRename", ws.pos("b.lisp", "helper", 0))
        .unwrap();
    assert_eq!(v["placeholder"], "helper");
    assert_eq!(v["range"]["start"]["character"], 6);

    let mut p = ws.pos("b.lisp", "helper", 0);
    p["newName"] = json!("assist");
    let v = c.request_raw("textDocument/rename", p).unwrap();
    let changes = v["changes"].as_object().unwrap();
    assert_eq!(changes.len(), 2);
    let b_edits = &changes[&ws.uri("b.lisp")];
    let starts: Vec<_> = b_edits
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["range"]["start"]["line"].as_u64().unwrap(),
                e["range"]["start"]["character"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(starts.len(), 2);
    assert!(starts.contains(&(0, 6)) && starts.contains(&(1, 1)));
    assert_eq!(changes[&ws.uri("a.lisp")][0]["newText"], "assist");
    c.shutdown();
}

#[test]
fn rename_local_and_validation() {
    let ws = Workspace::new(&[("a.lisp", "(let ((x 1)) (car x))")]);
    let mut c = ws.client(0);
    ws.open(&mut c, "a.lisp");
    let mut p = ws.pos("a.lisp", "x", 0);
    p["newName"] = json!("y");
    let v = c.request_raw("textDocument/rename", p.clone()).unwrap();
    assert_eq!(v["changes"][ws.uri("a.lisp")].as_array().unwrap().len(), 2);

    p["newName"] = json!("two words");
    assert_eq!(
        c.request_raw("textDocument/rename", p.clone())
            .unwrap_err()
            .0,
        -32602
    );
    p["newName"] = json!("(x)");
    assert_eq!(
        c.request_raw("textDocument/rename", p).unwrap_err().0,
        -32602
    );

    let err = c
        .request_raw("textDocument/prepareRename", ws.pos("a.lisp", "car", 0))
        .unwrap_err();
    assert!(err.1.contains("no definition"), "{}", err.1);
    c.shutdown();
}

#[test]
fn malformed_params_are_invalid_params() {
    let ws = Workspace::new(&[]);
    let mut c = ws.client(0);
    let err = c
        .request_raw(
            "textDocument/definition",
            json!({"textDocument": {"uri": "file:///x.lisp"}}),
        )
        .unwrap_err();
    assert_eq!(err.0, -32602);
    c.shutdown();
}

fn edits(v: &Value) -> Vec<(String, u64, u64)> {
    let mut out = Vec::new();
    for (uri, list) in v["changes"].as_object().unwrap() {
        let name = uri.rsplit('/').next().unwrap();
        for e in list.as_array().unwrap() {
            let start = &e["range"]["start"];
            out.push((
                name.to_owned(),
                start["line"].as_u64().unwrap(),
                start["character"].as_u64().unwrap(),
            ));
        }
    }
    out.sort();
    out
}

fn named(v: &[(&str, u64, u64)]) -> Vec<(String, u64, u64)> {
    v.iter().map(|&(f, l, c)| (f.to_owned(), l, c)).collect()
}

#[test]
fn rename_and_references_follow_clojure_namespaces() {
    let ws = Workspace::new(&[
        ("a.clj", "(ns app.a)\n(defn run [] 1)\n(run)"),
        ("b.clj", "(ns app.b)\n(defn run [] 2)"),
        (
            "c.clj",
            "(ns app.c (:require [app.a :as a] [app.b :refer [run]]))\n(a/run)\n(run)",
        ),
    ]);
    let mut c = ws.client(5);
    ws.open(&mut c, "a.clj");
    let mut p = ws.pos("a.clj", "run", 0);
    p["newName"] = json!("start");
    let v = c.request_raw("textDocument/rename", p).unwrap();
    assert_eq!(
        edits(&v),
        named(&[("a.clj", 1, 6), ("a.clj", 2, 1), ("c.clj", 1, 3)])
    );

    ws.open(&mut c, "c.clj");
    let mut p = ws.pos("c.clj", "(run)", 0);
    p["position"]["character"] = json!(1);
    p["context"] = json!({"includeDeclaration": true});
    let v = c.request_raw("textDocument/references", p).unwrap();
    assert_eq!(
        ranges(&v)
            .into_iter()
            .map(|(f, l, s, _)| (f, l, s))
            .collect::<Vec<_>>(),
        named(&[("b.clj", 1, 6), ("c.clj", 0, 49), ("c.clj", 2, 1)])
    );
    c.shutdown();
}

#[test]
fn rename_refuses_ambiguous_packages() {
    let ws = Workspace::new(&[
        ("a.lisp", "(in-package :app)\n(defun helper () 1)"),
        ("b.lisp", "(in-package :lib)\n(defun helper () 2)"),
        ("c.lisp", "(helper)\n(app::helper)"),
    ]);
    let mut c = ws.client(2);
    ws.open(&mut c, "c.lisp");
    let err = c
        .request_raw("textDocument/prepareRename", ws.pos("c.lisp", "helper", 0))
        .unwrap_err();
    assert!(err.1.contains("several namespaces"), "{}", err.1);

    ws.open(&mut c, "a.lisp");
    let mut p = ws.pos("a.lisp", "helper", 0);
    p["newName"] = json!("assist");
    let v = c.request_raw("textDocument/rename", p).unwrap();
    assert_eq!(edits(&v), named(&[("a.lisp", 1, 7), ("c.lisp", 1, 6)]));
    c.shutdown();
}

#[test]
fn lispico_cells_cross_file() {
    let ws = Workspace::new(&[
        ("a.lisp", "(defun n () 2)"),
        ("b.lisp", "(def n 1)"),
        ("c.lisp", "(n) n"),
    ]);
    let layers = Layers {
        cli: toml::from_str("[files.associations]\n\"*.lisp\" = \"lispico-cl\"").unwrap(),
        ..Layers::default()
    };
    let mut c = ws.client_with(layers, 2);
    ws.open(&mut c, "c.lisp");
    let v = c
        .request_raw("textDocument/definition", ws.pos("c.lisp", "n", 0))
        .unwrap();
    assert_eq!(ranges(&v), [("a.lisp".into(), 0, 7, 8)]);
    let v = c
        .request_raw("textDocument/definition", ws.pos("c.lisp", "n", 1))
        .unwrap();
    assert_eq!(ranges(&v), [("b.lisp".into(), 0, 5, 6)]);

    let mut p = ws.pos("c.lisp", "n", 0);
    p["context"] = json!({"includeDeclaration": true});
    let v = c.request_raw("textDocument/references", p).unwrap();
    assert_eq!(
        ranges(&v),
        [("a.lisp".into(), 0, 7, 8), ("c.lisp".into(), 0, 1, 2)]
    );

    ws.open(&mut c, "a.lisp");
    let mut p = ws.pos("a.lisp", " n ", 0);
    p["position"]["character"] = json!(p["position"]["character"].as_u64().unwrap() + 1);
    p["newName"] = json!("m");
    let v = c.request_raw("textDocument/rename", p).unwrap();
    assert_eq!(edits(&v), named(&[("a.lisp", 0, 7), ("c.lisp", 0, 1)]));
    c.shutdown();
}
