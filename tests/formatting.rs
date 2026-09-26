mod support;

use serde_json::{Value, json};
use support::Workspace;

fn apply(text: &str, edits: &Value) -> String {
    let lines: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let off = |p: &Value| {
        lines[p["line"].as_u64().unwrap() as usize] + p["character"].as_u64().unwrap() as usize
    };
    let mut edits: Vec<_> = edits
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                off(&e["range"]["start"]),
                off(&e["range"]["end"]),
                e["newText"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    edits.sort();
    let mut out = text.to_owned();
    for (s, e, t) in edits.into_iter().rev() {
        out.replace_range(s..e, &t);
    }
    out
}

#[test]
fn formatting_uses_workspace_indent_hints() {
    let ws = Workspace::new(&[
        (
            "macros.el",
            "(defmacro my-block (name &rest body) (declare (indent 1)) body)",
        ),
        ("a.el", "(my-block x\nbody)\n(other x\nbody)"),
    ]);
    let mut c = ws.client(1);
    ws.open(&mut c, "a.el");
    let v = c
        .request_raw(
            "textDocument/formatting",
            json!({"textDocument": {"uri": ws.uri("a.el")}, "options": {"tabSize": 2, "insertSpaces": true}}),
        )
        .unwrap();
    assert_eq!(
        apply(&ws.text("a.el"), &v),
        "(my-block x\n  body)\n(other x\n       body)"
    );
    c.shutdown();
}

#[test]
fn range_formatting_only_touches_range() {
    let ws = Workspace::new(&[("a.lisp", "(defun f ()\na\nb)")]);
    let mut c = ws.client(1);
    ws.open(&mut c, "a.lisp");
    let v = c
        .request_raw(
            "textDocument/rangeFormatting",
            json!({"textDocument": {"uri": ws.uri("a.lisp")},
                   "range": {"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 2}},
                   "options": {"tabSize": 2, "insertSpaces": true}}),
        )
        .unwrap();
    assert_eq!(apply(&ws.text("a.lisp"), &v), "(defun f ()\na\n  b)");
    c.shutdown();
}
