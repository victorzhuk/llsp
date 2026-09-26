use std::path::PathBuf;

use anyhow::{Context, Result};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::notification::{self as notif, Notification as _};
use lsp_types::{
    InitializeParams, InitializeResult, MessageType, PositionEncodingKind,
    PublishDiagnosticsParams, ServerCapabilities, ServerInfo, ShowMessageParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Uri,
};
use rustc_hash::FxHashMap;
use serde::de::DeserializeOwned;

use crate::config::{Layers, Settings, json_to_table};
use crate::diagnostics::{self, Severity};
use crate::document::{Document, Encoding, uri_to_path};

pub struct Server {
    conn: Connection,
    layers: Layers,
    settings: Settings,
    enc: Encoding,
    #[allow(dead_code)]
    roots: Vec<PathBuf>,
    docs: FxHashMap<Uri, Document>,
}

/// Runs the protocol until `exit`. Returns whether `shutdown` was received first.
pub fn run(conn: Connection, mut layers: Layers) -> Result<bool> {
    let (id, params) = conn.initialize_start()?;
    let params: InitializeParams =
        serde_json::from_value(params).context("decode initialize params")?;

    let roots = workspace_roots(&params);
    if let Some(root) = roots.first()
        && let Err(e) = layers.load_project(root)
    {
        log::warn!("{e:#}");
    }
    if let Some(opts) = params.initialization_options.clone() {
        layers.init = json_to_table(opts).context("initializationOptions")?;
    }
    let settings = match layers.resolve() {
        Ok(s) => s,
        Err(e) => {
            let msg = format!("{e:#}");
            conn.sender
                .send(Response::new_err(id, ErrorCode::InvalidParams as i32, msg.clone()).into())?;
            anyhow::bail!(msg);
        }
    };

    let enc = negotiate_encoding(&params);
    let result = InitializeResult {
        capabilities: capabilities(enc),
        server_info: Some(ServerInfo {
            name: "llsp".into(),
            version: Some(env!("CARGO_PKG_VERSION").into()),
        }),
    };
    conn.initialize_finish(id, serde_json::to_value(result)?)?;

    let mut server = Server {
        conn,
        layers,
        settings,
        enc,
        roots,
        docs: FxHashMap::default(),
    };
    server.main_loop()
}

fn negotiate_encoding(params: &InitializeParams) -> Encoding {
    let utf8 = params
        .capabilities
        .general
        .as_ref()
        .and_then(|g| g.position_encodings.as_ref())
        .is_some_and(|encs| encs.contains(&PositionEncodingKind::UTF8));
    if utf8 {
        Encoding::Utf8
    } else {
        Encoding::Utf16
    }
}

#[allow(deprecated)]
fn workspace_roots(params: &InitializeParams) -> Vec<PathBuf> {
    if let Some(folders) = &params.workspace_folders {
        return folders.iter().filter_map(|f| uri_to_path(&f.uri)).collect();
    }
    if let Some(uri) = &params.root_uri {
        return uri_to_path(uri).into_iter().collect();
    }
    params
        .root_path
        .as_ref()
        .map(PathBuf::from)
        .into_iter()
        .collect()
}

fn capabilities(enc: Encoding) -> ServerCapabilities {
    ServerCapabilities {
        position_encoding: Some(match enc {
            Encoding::Utf8 => PositionEncodingKind::UTF8,
            Encoding::Utf16 => PositionEncodingKind::UTF16,
        }),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                ..Default::default()
            },
        )),
        ..Default::default()
    }
}

impl Server {
    fn main_loop(&mut self) -> Result<bool> {
        while let Ok(msg) = self.conn.receiver.recv() {
            match msg {
                Message::Request(req) => {
                    if self.conn.handle_shutdown(&req)? {
                        return Ok(true);
                    }
                    self.on_request(req);
                }
                Message::Notification(n) if n.method == notif::Exit::METHOD => return Ok(false),
                Message::Notification(n) => self.on_notification(n),
                Message::Response(_) => {}
            }
        }
        Ok(false)
    }

    fn on_request(&mut self, req: Request) {
        let resp = Response::new_err(
            req.id,
            ErrorCode::MethodNotFound as i32,
            format!("unknown method {}", req.method),
        );
        self.send(resp.into());
    }

    fn on_notification(&mut self, n: Notification) {
        let method = n.method.clone();
        let result = match method.as_str() {
            notif::DidOpenTextDocument::METHOD => {
                self.notify::<notif::DidOpenTextDocument>(n, Self::did_open)
            }
            notif::DidChangeTextDocument::METHOD => {
                self.notify::<notif::DidChangeTextDocument>(n, Self::did_change)
            }
            notif::DidCloseTextDocument::METHOD => {
                self.notify::<notif::DidCloseTextDocument>(n, Self::did_close)
            }
            notif::DidChangeConfiguration::METHOD => {
                self.notify::<notif::DidChangeConfiguration>(n, Self::did_change_configuration)
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            log::warn!("{method}: {e:#}");
        }
    }

    fn notify<N>(&mut self, n: Notification, f: impl FnOnce(&mut Self, N::Params)) -> Result<()>
    where
        N: notif::Notification,
        N::Params: DeserializeOwned,
    {
        let params = serde_json::from_value(n.params)?;
        f(self, params);
        Ok(())
    }

    fn send(&self, msg: Message) {
        if self.conn.sender.send(msg).is_err() {
            log::error!("client connection closed");
        }
    }

    fn send_notification<N: notif::Notification>(&self, params: N::Params) {
        self.send(Notification::new(N::METHOD.to_owned(), params).into());
    }

    fn show_warning(&self, message: String) {
        log::warn!("{message}");
        self.send_notification::<notif::ShowMessage>(ShowMessageParams {
            typ: MessageType::WARNING,
            message,
        });
    }

    fn did_open(&mut self, p: lsp_types::DidOpenTextDocumentParams) {
        let doc = p.text_document;
        let path = uri_to_path(&doc.uri);
        let dialect = self
            .settings
            .detect(path.as_deref(), Some(&doc.language_id), &doc.text);
        let document = Document::new(doc.text, doc.version, dialect);
        self.docs.insert(doc.uri.clone(), document);
        self.publish_diagnostics(&doc.uri);
    }

    fn did_change(&mut self, p: lsp_types::DidChangeTextDocumentParams) {
        let uri = p.text_document.uri;
        let Some(doc) = self.docs.get_mut(&uri) else {
            log::warn!("change for unopened document {}", uri.as_str());
            return;
        };
        doc.apply_changes(p.content_changes, p.text_document.version, self.enc);
        self.publish_diagnostics(&uri);
    }

    fn did_close(&mut self, p: lsp_types::DidCloseTextDocumentParams) {
        let uri = p.text_document.uri;
        self.docs.remove(&uri);
        self.send_notification::<notif::PublishDiagnostics>(PublishDiagnosticsParams {
            uri,
            diagnostics: Vec::new(),
            version: None,
        });
    }

    fn did_change_configuration(&mut self, p: lsp_types::DidChangeConfigurationParams) {
        let settings = match p.settings {
            serde_json::Value::Object(mut o) if o.contains_key("llsp") => o.remove("llsp").unwrap(),
            other => other,
        };
        let mut layers = self.layers.clone();
        let resolved = json_to_table(settings).and_then(|t| {
            layers.client = t;
            layers.resolve()
        });
        match resolved {
            Ok(settings) => {
                self.layers = layers;
                self.settings = settings;
                self.reload_documents();
            }
            Err(e) => self.show_warning(format!("llsp: configuration ignored: {e:#}")),
        }
    }

    fn reload_documents(&mut self) {
        let uris: Vec<Uri> = self.docs.keys().cloned().collect();
        for uri in uris {
            let doc = &self.docs[&uri];
            let path = uri_to_path(&uri);
            let dialect = self.settings.detect(path.as_deref(), None, doc.text());
            let doc = Document::new(doc.text().to_owned(), doc.version, dialect);
            self.docs.insert(uri.clone(), doc);
            self.publish_diagnostics(&uri);
        }
    }

    fn publish_diagnostics(&self, uri: &Uri) {
        let Some(doc) = self.docs.get(uri) else {
            return;
        };
        let diagnostics = diagnostics::syntax(doc.tree())
            .into_iter()
            .map(|d| lsp_types::Diagnostic {
                range: doc.range(d.start, d.end, self.enc),
                severity: Some(match d.severity {
                    Severity::Error => lsp_types::DiagnosticSeverity::ERROR,
                    Severity::Warning => lsp_types::DiagnosticSeverity::WARNING,
                    Severity::Info => lsp_types::DiagnosticSeverity::INFORMATION,
                    Severity::Hint => lsp_types::DiagnosticSeverity::HINT,
                }),
                code: Some(lsp_types::NumberOrString::String(d.code.into())),
                source: Some("llsp".into()),
                message: d.message,
                ..Default::default()
            })
            .collect();
        self.send_notification::<notif::PublishDiagnostics>(PublishDiagnosticsParams {
            uri: uri.clone(),
            diagnostics,
            version: Some(doc.version),
        });
    }
}
