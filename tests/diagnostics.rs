mod support;

use llsp::config::Layers;
use serde_json::{Value, json};
use support::{Client, Workspace};

fn codes(d: &[Value]) -> Vec<String> {
    d.iter()
        .map(|x| x["code"].as_str().unwrap().to_owned())
        .collect()
}

fn layers(toml: &str) -> Layers {
    Layers {
        cli: toml::from_str(toml).unwrap(),
        ..Layers::default()
    }
}

#[test]
fn lints_published_with_tags() {
    let mut c = Client::start();
    c.open(
        "file:///w/a.lisp",
        "lisp",
        "(let ((a 1) (b 2)) a)\n(defun f ())\n(defun f ())",
    );
    let d = c.diagnostics("file:///w/a.lisp");
    assert_eq!(codes(&d), ["unused-binding", "duplicate-definition"]);
    assert_eq!(d[0]["severity"], 4);
    assert_eq!(d[0]["tags"], json!([1]));
    assert_eq!(d[1]["severity"], 2);
    c.shutdown();
}

#[test]
fn diagnostics_disabled_and_lint_off() {
    let mut c = Client::with(layers("[diagnostics]\nenable = false"), json!({}));
    c.open("file:///w/a.lisp", "lisp", "(let ((a 1)) (");
    assert!(c.diagnostics("file:///w/a.lisp").is_empty());
    c.shutdown();

    let mut c = Client::with(layers("[diagnostics]\nunused_binding = \"off\""), json!({}));
    c.open("file:///w/a.lisp", "lisp", "(let ((a 1)) 2)");
    assert!(c.diagnostics("file:///w/a.lisp").is_empty());
    c.shutdown();
}

#[test]
fn republished_after_index_loads() {
    let ws = Workspace::new(&[
        ("a.lisp", "(defun helper () 1)"),
        ("b.lisp", "(helper) (nowhere)"),
    ]);
    let l = layers("[diagnostics]\nunresolved_call = \"warning\"\ndebounce_ms = 0");
    let root = llsp::document::path_to_uri(&ws.root()).unwrap();
    let mut c = Client::with(l, json!({"rootUri": root.as_str()}));
    ws.open(&mut c, "b.lisp");
    let uri = ws.uri("b.lisp");
    let mut d = c.diagnostics(&uri);
    if d.len() == 2 {
        d = c.diagnostics(&uri);
    }
    assert_eq!(codes(&d), ["unresolved-call"]);
    assert_eq!(d[0]["message"], "`nowhere` is not defined in the workspace");
    c.shutdown();
}

#[test]
fn debounce_coalesces_changes() {
    let mut c = Client::with(layers("[diagnostics]\ndebounce_ms = 200"), json!({}));
    let uri = "file:///w/a.lisp";
    c.open(uri, "lisp", "(");
    for v in 2..6 {
        c.notify_raw(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri, "version": v}, "contentChanges": [{"text": "()"}]}),
        );
    }
    let p = c.wait_for("textDocument/publishDiagnostics");
    assert_eq!(p["version"], 5);
    assert!(p["diagnostics"].as_array().unwrap().is_empty());
    c.shutdown();
}

#[test]
fn lispico_value_binding_call_unresolved() {
    let l = layers(
        "[files.associations]\n\"*.lisp\" = \"lispico-cl\"\n[diagnostics]\nunused_binding = \"off\"\nunresolved_call = \"warning\"",
    );
    let mut c = Client::with(l, json!({}));
    c.open("file:///w/a.lisp", "lisp", "(let ((k 1)) (k))");
    let d = c.diagnostics("file:///w/a.lisp");
    assert_eq!(codes(&d), ["unresolved-call"]);
    assert_eq!(d[0]["message"], "`k` is not defined in the workspace");
    c.shutdown();

    let ws = Workspace::new(&[("a.lisp", "(def n 1)"), ("b.lisp", "(n)")]);
    let l = layers(
        "[files.associations]\n\"*.lisp\" = \"lispico-cl\"\n[diagnostics]\nunused_binding = \"off\"\nunresolved_call = \"warning\"\ndebounce_ms = 0",
    );
    let root = llsp::document::path_to_uri(&ws.root()).unwrap();
    let mut c = Client::with(l, json!({"rootUri": root.as_str()}));
    ws.open(&mut c, "b.lisp");
    let uri = ws.uri("b.lisp");
    let mut d = c.diagnostics(&uri);
    if d.is_empty() {
        d = c.diagnostics(&uri);
    }
    assert_eq!(codes(&d), ["unresolved-call"]);
    assert_eq!(d[0]["message"], "`n` is not defined in the workspace");
    c.shutdown();
}
