mod support;

use llsp::config::Layers;
use serde_json::json;
use support::Client;

const URI: &str = "file:///w/a.lisp";

fn sorted_codes_ranges(d: &[serde_json::Value]) -> Vec<(String, u64, u64)> {
    let mut v: Vec<(String, u64, u64)> = d
        .iter()
        .map(|d| {
            assert_eq!(d["range"]["start"]["line"], json!(0));
            assert_eq!(d["range"]["end"]["line"], json!(0));
            (
                d["code"].as_str().unwrap().to_string(),
                d["range"]["start"]["character"].as_u64().unwrap(),
                d["range"]["end"]["character"].as_u64().unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

#[test]
fn open_publishes_syntax_diagnostics() {
    let mut c = Client::start();
    c.open(URI, "lisp", "(defun f (x) x");
    let d = c.diagnostics(URI);
    assert_eq!(d.len(), 1, "{d:?}");
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
fn language_id_survives_configuration_reload() {
    for (id, uri, text, expected) in [
        (
            "lispico-clojure",
            "file:///w/rules/a",
            "#(1 2)",
            vec![("invalid-syntax", 0, 1)],
        ),
        (
            "lispico-cl",
            "file:///w/rules/b",
            "(f [x])",
            vec![("invalid-syntax", 3, 4), ("invalid-syntax", 5, 6)],
        ),
    ] {
        let expected = {
            let mut v: Vec<(String, u64, u64)> = expected
                .into_iter()
                .map(|(c, s, e)| (c.to_string(), s, e))
                .collect();
            v.sort();
            v
        };

        let mut c = Client::start();
        c.open(uri, id, text);
        assert_eq!(
            sorted_codes_ranges(&c.diagnostics(uri)),
            expected,
            "{id} baseline"
        );
        // Any settings change reloads open documents; the client ID must stick.
        for _ in 0..3 {
            c.notify_raw("workspace/didChangeConfiguration", json!({"settings": {}}));
            assert_eq!(
                sorted_codes_ranges(&c.diagnostics(uri)),
                expected,
                "{id} after reload"
            );
        }
        c.shutdown();
    }

    // A configured association overrides the retained client ID.
    let uri = "file:///w/rules/a";
    let mut c = Client::start();
    c.open(uri, "lispico-clojure", "#(1 2)");
    assert_eq!(
        sorted_codes_ranges(&c.diagnostics(uri)),
        vec![("invalid-syntax".into(), 0, 1)],
        "clojure baseline"
    );
    c.notify_raw(
        "workspace/didChangeConfiguration",
        json!({"settings": {"files": {"associations": {"**/rules/a": "lispico-cl"}}}}),
    );
    assert!(
        c.diagnostics(uri).is_empty(),
        "#(1 2) is a valid vector in lispico-cl"
    );
    // Removing the association falls back to the retained client ID.
    c.notify_raw("workspace/didChangeConfiguration", json!({"settings": {}}));
    assert_eq!(
        sorted_codes_ranges(&c.diagnostics(uri)),
        vec![("invalid-syntax".into(), 0, 1)],
        "clojure diagnostics return without association"
    );
    for _ in 0..3 {
        c.notify_raw("workspace/didChangeConfiguration", json!({"settings": {}}));
        assert_eq!(
            sorted_codes_ranges(&c.diagnostics(uri)),
            vec![("invalid-syntax".into(), 0, 1)],
            "clojure diagnostics persist across repeated reloads"
        );
    }
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
