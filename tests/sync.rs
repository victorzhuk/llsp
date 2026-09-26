mod support;

use llsp::config::Layers;
use serde_json::json;
use support::Client;

const URI: &str = "file:///w/a.lisp";

#[test]
fn open_publishes_syntax_diagnostics() {
    let mut c = Client::start();
    c.open(URI, "lisp", "(defun f (x)");
    let d = c.diagnostics(URI);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0]["code"], "unclosed-delimiter");
    assert_eq!(d[0]["range"]["start"], json!({"line": 0, "character": 0}));
    c.shutdown();
}

#[test]
fn incremental_change_fixes_error() {
    let mut c = Client::start();
    c.open(URI, "lisp", "(a b");
    assert_eq!(c.diagnostics(URI).len(), 1);
    c.notify_raw(
        "textDocument/didChange",
        json!({
            "textDocument": {"uri": URI, "version": 2},
            "contentChanges": [{"range": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 4}}, "text": "c d)"}]
        }),
    );
    assert!(c.diagnostics(URI).is_empty());
    c.shutdown();
}

#[test]
fn utf16_positions_in_diagnostics() {
    let mut c = Client::start();
    c.open(URI, "lisp", "(ö 😀 \"x");
    let d = c.diagnostics(URI);
    let string_err = d
        .iter()
        .find(|d| d["code"] == "unterminated-string")
        .unwrap();
    assert_eq!(string_err["range"]["start"]["character"], 6);
    c.shutdown();
}

#[test]
fn close_clears_diagnostics() {
    let mut c = Client::start();
    c.open(URI, "lisp", "(");
    assert_eq!(c.diagnostics(URI).len(), 1);
    c.notify_raw(
        "textDocument/didClose",
        json!({"textDocument": {"uri": URI}}),
    );
    assert!(c.diagnostics(URI).is_empty());
    c.shutdown();
}

#[test]
fn dialect_from_association_and_modeline() {
    let layers = Layers {
        cli: toml::from_str("[files.associations]\n\"*.lsp\" = \"clojure\"").unwrap(),
        ..Layers::default()
    };
    let mut c = Client::with(layers, json!({}));
    c.open("file:///w/a.lsp", "lisp", "{:a 1}");
    assert!(
        c.diagnostics("file:///w/a.lsp").is_empty(),
        "braces valid in clojure"
    );
    c.open(
        "file:///w/script",
        "plaintext",
        ";; -*- mode: clojure -*-\n[1 2]",
    );
    assert!(c.diagnostics("file:///w/script").is_empty());
    c.open("file:///w/b.lisp", "lisp", "{:a 1}");
    assert!(
        c.diagnostics("file:///w/b.lisp").is_empty(),
        "braces are constituents in CL"
    );
    c.shutdown();
}

#[test]
fn bad_client_settings_are_ignored_with_warning() {
    let mut c = Client::start();
    c.notify_raw(
        "workspace/didChangeConfiguration",
        json!({"settings": {"llsp": {"fromat": {}}}}),
    );
    let msg = c.wait_for("window/showMessage");
    assert!(msg["message"].as_str().unwrap().contains("fromat"));
    c.open(URI, "lisp", "(");
    assert_eq!(c.diagnostics(URI).len(), 1, "server still works");
    c.shutdown();
}

#[test]
fn client_settings_apply() {
    let mut c = Client::start();
    c.open("file:///w/x.foo", "plaintext", "{:a 1");
    assert!(
        c.diagnostics("file:///w/x.foo").is_empty(),
        "braces are constituents in CL"
    );
    c.notify_raw(
        "workspace/didChangeConfiguration",
        json!({"settings": {"files": {"default_dialect": "clojure"}}}),
    );
    let d = c.diagnostics("file:///w/x.foo");
    assert_eq!(d[0]["code"], "unclosed-delimiter");
    c.shutdown();
}
