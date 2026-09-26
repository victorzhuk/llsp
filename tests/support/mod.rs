#![allow(dead_code)]

use std::str::FromStr;
use std::thread::JoinHandle;
use std::time::Duration;

use llsp::config::Layers;
use lsp_server::{Connection, Message, Notification, Request, RequestId, Response};
use lsp_types::{Position, TextDocumentIdentifier, TextDocumentPositionParams, Uri};
use serde_json::{Value, json};

pub struct Client {
    conn: Connection,
    server: Option<JoinHandle<anyhow::Result<bool>>>,
    next_id: i32,
    pub notifications: Vec<Notification>,
    pub init: Value,
}

const TIMEOUT: Duration = Duration::from_secs(10);

impl Client {
    pub fn start() -> Self {
        Self::with(Layers::default(), json!({}))
    }

    pub fn with(layers: Layers, init_params: Value) -> Self {
        let (server_conn, conn) = Connection::memory();
        let server = std::thread::spawn(move || llsp::server::run(server_conn, layers));
        let mut client = Self {
            conn,
            server: Some(server),
            next_id: 0,
            notifications: Vec::new(),
            init: Value::Null,
        };
        let mut params = json!({"capabilities": {}});
        merge(&mut params, init_params);
        client.init = client
            .request_raw("initialize", params)
            .expect("initialize");
        client.notify_raw("initialized", json!({}));
        client
    }

    pub fn request_raw(&mut self, method: &str, params: Value) -> Result<Value, (i32, String)> {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        self.conn
            .sender
            .send(Request::new(id.clone(), method.into(), params).into())
            .unwrap();
        loop {
            match self.conn.receiver.recv_timeout(TIMEOUT).expect("response") {
                Message::Response(Response {
                    id: rid,
                    response_result,
                }) if rid == id => {
                    return response_result.map_err(|e| (e.code, e.message));
                }
                Message::Notification(n) => self.notifications.push(n),
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    pub fn request<R: lsp_types::request::Request>(&mut self, params: R::Params) -> R::Result {
        let v = self
            .request_raw(R::METHOD, serde_json::to_value(params).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e:?}", R::METHOD));
        serde_json::from_value(v).unwrap()
    }

    pub fn notify_raw(&mut self, method: &str, params: Value) {
        self.conn
            .sender
            .send(Notification::new(method.into(), params).into())
            .unwrap();
    }

    pub fn notify<N: lsp_types::notification::Notification>(&mut self, params: N::Params) {
        self.notify_raw(N::METHOD, serde_json::to_value(params).unwrap());
    }

    /// Waits for the next notification with `method`, collecting others.
    pub fn wait_for(&mut self, method: &str) -> Value {
        if let Some(i) = self.notifications.iter().position(|n| n.method == method) {
            return self.notifications.remove(i).params;
        }
        loop {
            match self.conn.receiver.recv_timeout(TIMEOUT).expect(method) {
                Message::Notification(n) if n.method == method => return n.params,
                Message::Notification(n) => self.notifications.push(n),
                other => panic!("unexpected {other:?}"),
            }
        }
    }

    pub fn diagnostics(&mut self, uri: &str) -> Vec<Value> {
        loop {
            let p = self.wait_for("textDocument/publishDiagnostics");
            if p["uri"] == uri {
                return p["diagnostics"].as_array().unwrap().clone();
            }
        }
    }

    pub fn open(&mut self, uri: &str, language_id: &str, text: &str) {
        self.notify_raw(
            "textDocument/didOpen",
            json!({"textDocument": {"uri": uri, "languageId": language_id, "version": 1, "text": text}}),
        );
    }

    pub fn shutdown(mut self) -> bool {
        self.request_raw("shutdown", Value::Null).unwrap();
        self.notify_raw("exit", Value::Null);
        self.server.take().unwrap().join().unwrap().unwrap()
    }
}

pub fn uri(s: &str) -> Uri {
    Uri::from_str(s).unwrap()
}

pub fn pos(uri_str: &str, line: u32, character: u32) -> TextDocumentPositionParams {
    TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: uri(uri_str) },
        position: Position::new(line, character),
    }
}

fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                merge(b.entry(k).or_insert(Value::Null), v);
            }
        }
        (b, o) => *b = o,
    }
}

pub struct Workspace {
    pub dir: tempfile::TempDir,
}

impl Workspace {
    pub fn new(files: &[(&str, &str)]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let p = dir.path().join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        Self { dir }
    }

    pub fn root(&self) -> std::path::PathBuf {
        self.dir.path().canonicalize().unwrap()
    }

    pub fn uri(&self, rel: &str) -> String {
        llsp::document::path_to_uri(&self.root().join(rel))
            .unwrap()
            .as_str()
            .to_owned()
    }

    pub fn text(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root().join(rel)).unwrap()
    }

    /// Starts a client rooted here and waits until `defs` definitions are indexed.
    pub fn client(&self, defs: usize) -> Client {
        self.client_with(Layers::default(), defs)
    }

    pub fn client_with(&self, layers: Layers, defs: usize) -> Client {
        let root = llsp::document::path_to_uri(&self.root()).unwrap();
        let mut c = Client::with(layers, json!({"rootUri": root.as_str()}));
        let deadline = std::time::Instant::now() + TIMEOUT;
        loop {
            let v = c
                .request_raw("workspace/symbol", json!({"query": ""}))
                .unwrap();
            if v.as_array().map_or(0, Vec::len) >= defs {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "index timeout: {v}");
            std::thread::sleep(Duration::from_millis(10));
        }
        c
    }

    /// Opens a workspace file with its on-disk text.
    pub fn open(&self, c: &mut Client, rel: &str) {
        let lang = match rel.rsplit('.').next() {
            Some("clj") => "clojure",
            Some("scm") => "scheme",
            Some("el") => "emacs-lisp",
            _ => "lisp",
        };
        c.open(&self.uri(rel), lang, &self.text(rel));
    }

    /// Position of the `n`-th occurrence of `needle` in `rel` (UTF-16 == bytes for ASCII).
    pub fn pos(&self, rel: &str, needle: &str, n: usize) -> Value {
        let text = self.text(rel);
        let off = text.match_indices(needle).nth(n).expect(needle).0;
        let line = text[..off].matches('\n').count();
        let col = off - text[..off].rfind('\n').map_or(0, |i| i + 1);
        json!({"textDocument": {"uri": self.uri(rel)}, "position": {"line": line, "character": col}})
    }
}
