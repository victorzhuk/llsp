mod support;

use llsp::config::Layers;
use proptest::prelude::*;
use serde_json::{Value, json};
use support::Client;

const DIALECTS: [(&str, &str); 9] = [
    ("lisp", "file:///w/a.lisp"),
    ("clojure", "file:///w/a.clj"),
    ("scheme", "file:///w/a.scm"),
    ("racket", "file:///w/a.rkt"),
    ("emacs-lisp", "file:///w/a.el"),
    ("fennel", "file:///w/a.fnl"),
    ("janet", "file:///w/a.janet"),
    ("lispico-clojure", "file:///w/a.lpc"),
    ("lispico-cl", "file:///w/b.lpc"),
];

const POSITION_REQUESTS: [&str; 8] = [
    "textDocument/definition",
    "textDocument/references",
    "textDocument/documentHighlight",
    "textDocument/hover",
    "textDocument/completion",
    "textDocument/signatureHelp",
    "textDocument/prepareRename",
    "textDocument/rename",
];

fn exercise(c: &mut Client, uri: &str, text: &str, positions: &[(u32, u32)]) {
    let doc = json!({"uri": uri});
    for method in [
        "textDocument/documentSymbol",
        "textDocument/foldingRange",
        "textDocument/semanticTokens/full",
        "textDocument/formatting",
    ] {
        let params = json!({"textDocument": doc, "options": {"tabSize": 2, "insertSpaces": true}});
        c.request_raw(method, params)
            .unwrap_or_else(|e| panic!("{method} on {text:?}: {e:?}"));
    }
    for &(line, character) in positions {
        let pos = json!({"line": line, "character": character});
        for method in POSITION_REQUESTS {
            let params = json!({
                "textDocument": doc,
                "position": pos,
                "context": {"includeDeclaration": true},
                "newName": "renamed",
            });
            match c.request_raw(method, params) {
                Ok(_) => {}
                Err((code, msg)) => assert!(
                    code == -32803 || code == -32602,
                    "{method} at {line}:{character} on {text:?}: {code} {msg}"
                ),
            }
        }
        for method in [
            "textDocument/selectionRange",
            "textDocument/semanticTokens/range",
            "textDocument/rangeFormatting",
        ] {
            let params = json!({
                "textDocument": doc,
                "positions": [pos],
                "range": {"start": {"line": 0, "character": 0}, "end": pos},
                "options": {"tabSize": 2, "insertSpaces": true},
            });
            c.request_raw(method, params)
                .unwrap_or_else(|e| panic!("{method} on {text:?}: {e:?}"));
        }
    }
    let _ = c
        .request_raw("workspace/symbol", json!({"query": "a"}))
        .unwrap();
}

fn lisp_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            Just("(".to_owned()),
            Just(")".to_owned()),
            Just("[".to_owned()),
            Just("]".to_owned()),
            Just("{".to_owned()),
            Just("}".to_owned()),
            Just("\n".to_owned()),
            Just("\r\n".to_owned()),
            Just("\r".to_owned()),
            Just(" ".to_owned()),
            Just("\"".to_owned()),
            Just("#".to_owned()),
            Just("'".to_owned()),
            Just(";".to_owned()),
            Just("|".to_owned()),
            Just("\\".to_owned()),
            Just("defun ".to_owned()),
            Just("let ".to_owned()),
            Just("defn ".to_owned()),
            Just("fn ".to_owned()),
            Just("&rest ".to_owned()),
            Just("ö😀".to_owned()),
            "[a-z:/.]{1,5}",
        ],
        0..40,
    )
    .prop_map(|v| v.concat())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn every_request_survives_random_documents(
        text in lisp_text(),
        d in 0usize..9,
        positions in proptest::collection::vec((0u32..4, 0u32..30), 1..4),
        close_and_reopen in proptest::bool::ANY,
        watched_outside in proptest::bool::ANY,
        invalid_rename in proptest::bool::ANY,
    ) {
        let (lang, uri) = DIALECTS[d];
        let mut c = Client::with(
            Layers {
                cli: toml::from_str("[diagnostics]\nunresolved_call = \"hint\"").unwrap(),
                ..Layers::default()
            },
            json!({}),
        );
        c.open(uri, lang, &text);
        exercise(&mut c, uri, &text, &positions);
        c.notify_raw(
            "textDocument/didChange",
            json!({"textDocument": {"uri": uri, "version": 2},
                   "contentChanges": [{"range": {"start": {"line": 0, "character": 1}, "end": {"line": 9, "character": 0}}, "text": ")("}]}),
        );
        exercise(&mut c, uri, &text, &positions);
        // State transitions around the document: close + reopen, watched-file
        // events (including outside the roots), and a settings reload.
        if close_and_reopen {
            c.notify_raw("textDocument/didClose", json!({"textDocument": {"uri": uri}}));
            c.request_raw("workspace/symbol", json!({"query": ""})).unwrap();
            c.open(uri, lang, &text);
        }
        c.notify_raw(
            "workspace/didChangeWatchedFiles",
            json!({"changes": [
                {"uri": uri, "type": 2},
                {"uri": "file:///outside/x.lisp", "type": if watched_outside { 1 } else { 3 }},
            ]}),
        );
        c.notify_raw(
            "workspace/didChangeConfiguration",
            json!({"settings": {"format": {"body_indent": 3}}}),
        );
        exercise(&mut c, uri, &text, &positions);
        if invalid_rename {
            // An invalid new name is a parameter error, not a crash.
            let err = c
                .request_raw(
                    "textDocument/rename",
                    json!({"textDocument": {"uri": uri}, "position": {"line": 0, "character": 0}, "newName": "two words"}),
                )
                .unwrap_err();
            prop_assert_eq!(err.0, -32602);
        }
        prop_assert!(c.shutdown());
    }
}

#[test]
fn state_transition_notifications_keep_server_running() {
    let mut c = Client::start();
    c.open("file:///w/a.lisp", "lisp", "(defun f () 1)");
    c.notify_raw(
        "textDocument/didClose",
        json!({"textDocument": {"uri": "file:///w/a.lisp"}}),
    );
    c.notify_raw(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [
            {"uri": "file:///w/a.lisp", "type": 2},
            {"uri": "file:///outside/x.lisp", "type": 1},
        ]}),
    );
    c.notify_raw(
        "workspace/didChangeConfiguration",
        json!({"settings": {"format": {"body_indent": 3}}}),
    );
    // Requests still answer and shutdown stays clean after the transitions.
    let _ = c
        .request_raw("workspace/symbol", json!({"query": "f"}))
        .unwrap();
    assert!(c.shutdown());
}

#[test]
fn read_eval_document_is_only_syntax() {
    let mut c = Client::start();
    let text = "#.(run-program \"rm\")\n(defun f () #.(error \"boom\"))";
    c.open("file:///w/a.lisp", "lisp", text);
    exercise(&mut c, "file:///w/a.lisp", text, &[(0, 3), (1, 9), (1, 16)]);
    let v = c
        .request_raw(
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": "file:///w/a.lisp"}}),
        )
        .unwrap();
    assert_eq!(v[0]["name"], "f");
    assert!(c.shutdown());
}

#[test]
fn oversized_document_is_not_analyzed() {
    let layers = Layers {
        cli: toml::from_str("[files]\nmax_file_size = 100").unwrap(),
        ..Layers::default()
    };
    let mut c = Client::with(layers, json!({}));
    let text = format!("(defun f (unused) {})", "(g) ".repeat(250));
    c.open("file:///w/big.lisp", "lisp", &text);
    let warning = c.wait_for("window/showMessage");
    assert!(
        warning["message"]
            .as_str()
            .unwrap()
            .contains("files.max_file_size")
    );
    assert!(c.diagnostics("file:///w/big.lisp").is_empty());
    let v = c
        .request_raw(
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": "file:///w/big.lisp"}}),
        )
        .unwrap();
    assert_eq!(v, Value::Null);
    let v = c
        .request_raw(
            "textDocument/formatting",
            json!({"textDocument": {"uri": "file:///w/big.lisp"}, "options": {"tabSize": 2, "insertSpaces": true}}),
        )
        .unwrap();
    assert_eq!(v, Value::Null);
    assert!(c.shutdown());
}
