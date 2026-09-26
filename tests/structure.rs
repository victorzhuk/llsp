mod support;

use serde_json::{Value, json};
use support::Client;

const TYPES: [&str; 12] = [
    "namespace",
    "type",
    "function",
    "macro",
    "variable",
    "parameter",
    "property",
    "keyword",
    "comment",
    "string",
    "number",
    "regexp",
];
const MODS: [&str; 4] = ["declaration", "definition", "readonly", "defaultLibrary"];

/// Decodes semantic tokens into (text, type, modifiers).
fn decode(text: &str, data: &Value) -> Vec<(String, String, Vec<String>)> {
    let lines: Vec<&str> = text.split('\n').collect();
    let d: Vec<u64> = data
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap())
        .collect();
    let (mut line, mut col) = (0usize, 0usize);
    d.chunks(5)
        .map(|c| {
            if c[0] > 0 {
                line += c[0] as usize;
                col = c[1] as usize;
            } else {
                col += c[1] as usize;
            }
            let tok: String = lines[line].chars().skip(col).take(c[2] as usize).collect();
            let mods = MODS
                .iter()
                .enumerate()
                .filter(|(i, _)| c[4] & (1 << i) != 0)
                .map(|(_, m)| m.to_string())
                .collect();
            (tok, TYPES[c[3] as usize].to_owned(), mods)
        })
        .collect()
}

fn tokens(c: &mut Client, uri: &str, text: &str) -> Vec<(String, String, Vec<String>)> {
    let v = c
        .request_raw(
            "textDocument/semanticTokens/full",
            json!({"textDocument": {"uri": uri}}),
        )
        .unwrap();
    decode(text, &v["data"])
}

fn t(s: &str, ty: &str, mods: &[&str]) -> (String, String, Vec<String>) {
    (
        s.into(),
        ty.into(),
        mods.iter().map(|m| m.to_string()).collect(),
    )
}

#[test]
fn semantic_classification() {
    let mut c = Client::start();
    let text = "(defun f (x) (car x)) ; c";
    c.open("file:///w/a.lisp", "lisp", text);
    assert_eq!(
        tokens(&mut c, "file:///w/a.lisp", text),
        [
            t("defun", "keyword", &[]),
            t("f", "function", &["definition"]),
            t("x", "parameter", &["declaration"]),
            t("car", "function", &["defaultLibrary"]),
            t("x", "parameter", &[]),
            t("; c", "comment", &[]),
        ]
    );
    c.shutdown();
}

#[test]
fn semantic_literals_and_namespaces() {
    let mut c = Client::start();
    let text =
        "(ns a (:require [b :as bb]))\n(bb/go :k 1 nil #\"re\" \\x \"two\nlines\" #_ (skip))";
    c.open("file:///w/a.clj", "clojure", text);
    let toks = tokens(&mut c, "file:///w/a.clj", text);
    let find = |s: &str| {
        toks.iter()
            .find(|x| x.0 == s)
            .unwrap_or_else(|| panic!("{s}: {toks:?}"))
            .clone()
    };
    assert_eq!(find("bb").1, "namespace");
    assert_eq!(find(":k").1, "property");
    assert_eq!(find("1").1, "number");
    assert_eq!(
        find("nil"),
        t("nil", "variable", &["readonly", "defaultLibrary"])
    );
    assert_eq!(find("\"re\"").1, "regexp");
    assert_eq!(find("\\x").1, "string");
    assert_eq!(find("\"two").1, "string");
    assert_eq!(find("lines\"").1, "string");
    assert_eq!(find("#_ (skip)").1, "comment");
    c.shutdown();
}

#[test]
fn semantic_tokens_utf16_and_range() {
    let mut c = Client::start();
    let text = "(let ((ö 1)) ö)";
    c.open("file:///w/a.lisp", "lisp", text);
    let toks = tokens(&mut c, "file:///w/a.lisp", text);
    assert_eq!(toks[1], t("ö", "parameter", &["declaration"]));
    assert_eq!(toks.last().unwrap(), &t("ö", "parameter", &[]));
    let v = c
        .request_raw(
            "textDocument/semanticTokens/range",
            json!({"textDocument": {"uri": "file:///w/a.lisp"},
                   "range": {"start": {"line": 0, "character": 12}, "end": {"line": 0, "character": 15}}}),
        )
        .unwrap();
    assert_eq!(v["data"].as_array().unwrap().len(), 5, "only the last ö");
    c.shutdown();
}

#[test]
fn folding_ranges() {
    let mut c = Client::start();
    let text = ";; a\n;; b\n(defun f ()\n  (let ((x 1))\n    x))\n#| one\ntwo |#";
    c.open("file:///w/a.lisp", "lisp", text);
    let v = c
        .request_raw(
            "textDocument/foldingRange",
            json!({"textDocument": {"uri": "file:///w/a.lisp"}}),
        )
        .unwrap();
    let folds: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["startLine"].as_u64().unwrap(),
                f["endLine"].as_u64().unwrap(),
                f["kind"].clone(),
            )
        })
        .collect();
    assert_eq!(
        folds,
        [
            (0, 1, json!("comment")),
            (2, 4, Value::Null),
            (3, 4, Value::Null),
            (5, 6, json!("comment"))
        ]
    );
    c.shutdown();
}

#[test]
fn selection_ranges() {
    let mut c = Client::start();
    c.open("file:///w/a.lisp", "lisp", "(a (b c))");
    let v = c
        .request_raw(
            "textDocument/selectionRange",
            json!({"textDocument": {"uri": "file:///w/a.lisp"}, "positions": [{"line": 0, "character": 4}]}),
        )
        .unwrap();
    let mut chain = Vec::new();
    let mut cur = &v[0];
    while !cur.is_null() {
        chain.push((
            cur["range"]["start"]["character"].as_u64().unwrap(),
            cur["range"]["end"]["character"].as_u64().unwrap(),
        ));
        cur = &cur["parent"];
    }
    assert_eq!(chain, [(4, 5), (3, 8), (0, 9)]);
    c.shutdown();
}
