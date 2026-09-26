use std::path::Path;
use std::time::Duration;

use criterion::{Criterion, criterion_group, criterion_main};
use llsp::config::Layers;
use llsp::document::path_to_uri;
use lsp_server::{Connection, Message, Notification, Request, RequestId};
use serde_json::{Value, json};

const FILES: usize = 1000;
const DEFS_PER_FILE: usize = 50;

fn write_workspace(root: &Path) {
    for f in 0..FILES {
        let mut src = format!("(in-package :pkg{f})\n\n");
        for d in 0..DEFS_PER_FILE {
            let callee = (d + 1) % DEFS_PER_FILE;
            src.push_str(&format!(
                "(defun fn-{f}-{d} (a b &optional (c 1))\n  \"Doc {d}.\"\n  (let ((x (+ a b c)))\n    (shared-helper (fn-{f}-{callee} x b) x)))\n\n"
            ));
        }
        std::fs::write(root.join(format!("f{f}.lisp")), src).unwrap();
    }
    std::fs::write(
        root.join("helper.lisp"),
        "(defun shared-helper (x y) \"Shared.\" (list x y))\n",
    )
    .unwrap();
}

struct Client {
    conn: Connection,
    id: i32,
}

impl Client {
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let id = RequestId::from(self.id);
        self.conn
            .sender
            .send(Request::new(id.clone(), method.into(), params).into())
            .unwrap();
        loop {
            match self
                .conn
                .receiver
                .recv_timeout(Duration::from_secs(30))
                .unwrap()
            {
                Message::Response(r) if r.id == id => return r.response_result.unwrap(),
                _ => {}
            }
        }
    }

    fn notify(&self, method: &str, params: Value) {
        self.conn
            .sender
            .send(Notification::new(method.into(), params).into())
            .unwrap();
    }
}

fn workspace(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    write_workspace(&root);
    let settings = Layers::default().resolve().unwrap();

    c.bench_function("index_cold_1000_files", |b| {
        b.iter(|| llsp::workspace::scan(&settings, std::slice::from_ref(&root)))
    });

    let (server_conn, conn) = Connection::memory();
    std::thread::spawn(move || llsp::server::run(server_conn, Layers::default()));
    let mut client = Client { conn, id: 0 };
    let root_uri = path_to_uri(&root).unwrap();
    client.request(
        "initialize",
        json!({"capabilities": {"general": {"positionEncodings": ["utf-16"]}}, "rootUri": root_uri.as_str()}),
    );
    client.notify("initialized", json!({}));
    let total = FILES * DEFS_PER_FILE + 1;
    loop {
        let v = client.request("workspace/symbol", json!({"query": "shared-helper"}));
        if !v.as_array().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    let path = root.join("f0.lisp");
    let uri = path_to_uri(&path).unwrap().as_str().to_owned();
    let text = std::fs::read_to_string(&path).unwrap();
    client.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "lisp", "version": 1, "text": text}}),
    );
    let doc = json!({"uri": uri});
    let at = |line: u32, character: u32| json!({"textDocument": doc, "position": {"line": line, "character": character}});

    let mut group = c.benchmark_group(format!("requests_{total}_defs"));
    group.bench_function("completion", |b| {
        b.iter(|| client.request("textDocument/completion", at(5, 10)))
    });
    group.bench_function("definition", |b| {
        b.iter(|| client.request("textDocument/definition", at(5, 6)))
    });
    group.bench_function("references_shared", |b| {
        let mut p = at(5, 6);
        p["context"] = json!({"includeDeclaration": true});
        b.iter(|| client.request("textDocument/references", p.clone()))
    });
    group.bench_function("references_typical", |b| {
        let mut p = at(5, 24);
        p["context"] = json!({"includeDeclaration": true});
        b.iter(|| client.request("textDocument/references", p.clone()))
    });
    group.bench_function("hover", |b| {
        b.iter(|| client.request("textDocument/hover", at(5, 6)))
    });
    group.bench_function("workspace_symbol", |b| {
        b.iter(|| client.request("workspace/symbol", json!({"query": "fn-42-4"})))
    });
    group.bench_function("semantic_tokens_full", |b| {
        b.iter(|| {
            client.request(
                "textDocument/semanticTokens/full",
                json!({"textDocument": doc}),
            )
        })
    });
    group.bench_function("formatting", |b| {
        b.iter(|| {
            client.request(
                "textDocument/formatting",
                json!({"textDocument": doc, "options": {"tabSize": 2, "insertSpaces": true}}),
            )
        })
    });
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20).measurement_time(Duration::from_secs(4));
    targets = workspace
}
criterion_main!(benches);
