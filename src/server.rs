use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossbeam_channel::{Receiver, Sender, select};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{self as notif, Notification as _};
use lsp_types::request::{self as req, Request as _};
use lsp_types::{
    CompletionOptions, DidChangeWatchedFilesRegistrationOptions, FileChangeType, FileSystemWatcher,
    FoldingRangeProviderCapability, GlobPattern, HoverProviderCapability, InitializeParams,
    InitializeResult, MessageType, OneOf, PositionEncodingKind, PublishDiagnosticsParams,
    Registration, RegistrationParams, RenameOptions, SelectionRangeProviderCapability,
    SemanticTokensFullOptions, SemanticTokensOptions, SemanticTokensServerCapabilities,
    ServerCapabilities, ServerInfo, ShowMessageParams, SignatureHelpOptions,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, Uri,
};
use rustc_hash::FxHashMap;
use serde::de::DeserializeOwned;

use crate::config::Layers;
use crate::diagnostics::{self, Severity};
use crate::dialect::Cell;
use crate::document::{Document, Encoding, normalize_uri, uri_to_path};
use crate::features::{
    FeatureError, FeatureResult, assist, formatting, navigation, structure, symbols,
};
use crate::session::Session;
use crate::tables::json_to_table;
use crate::workspace::{self, FileSummary, Index};

pub struct Server {
    session: Session,
    conn: Connection,
    layers: Layers,
    events: (Sender<Event>, Receiver<Event>),
    scan_generation: u64,
    watch_files: bool,
    next_request: i32,
    pending: FxHashMap<Uri, Instant>,
}

enum Event {
    Indexed(u64, Vec<FileSummary>),
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

    let watch_files = params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|w| w.did_change_watched_files.as_ref())
        .and_then(|w| w.dynamic_registration)
        .unwrap_or(false);
    let mut server = Server {
        session: Session {
            settings,
            enc,
            roots: roots.iter().filter_map(|r| r.canonicalize().ok()).collect(),
            docs: FxHashMap::default(),
            index: Index::default(),
        },
        conn,
        layers,
        events: crossbeam_channel::unbounded(),
        scan_generation: 0,
        watch_files,
        next_request: 0,
        pending: FxHashMap::default(),
    };
    server.register_watchers();
    server.start_scan();
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
        document_symbol_provider: Some(OneOf::Left(true)),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_highlight_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec!["/".into(), ":".into()]),
            ..Default::default()
        }),
        signature_help_provider: Some(SignatureHelpOptions {
            trigger_characters: Some(vec![" ".into(), "(".into()]),
            retrigger_characters: None,
            work_done_progress_options: Default::default(),
        }),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
        document_formatting_provider: Some(OneOf::Left(true)),
        document_range_formatting_provider: Some(OneOf::Left(true)),
        selection_range_provider: Some(SelectionRangeProviderCapability::Simple(true)),
        semantic_tokens_provider: Some(SemanticTokensServerCapabilities::SemanticTokensOptions(
            SemanticTokensOptions {
                legend: crate::features::legend(),
                range: Some(true),
                full: Some(SemanticTokensFullOptions::Bool(true)),
                work_done_progress_options: Default::default(),
            },
        )),
        ..Default::default()
    }
}

impl Server {
    fn main_loop(&mut self) -> Result<bool> {
        let events = self.events.1.clone();
        loop {
            let timer = match self.pending.values().min() {
                Some(&at) => crossbeam_channel::at(at),
                None => crossbeam_channel::never(),
            };
            select! {
                recv(self.conn.receiver) -> msg => {
                    let Ok(msg) = msg else { return Ok(false) };
                    match msg {
                        Message::Request(req) => {
                            if self.conn.handle_shutdown(&req)? {
                                return Ok(true);
                            }
                            self.on_request(req);
                        }
                        Message::Notification(n) if n.method == notif::Exit::METHOD => {
                            return Ok(false);
                        }
                        Message::Notification(n) => self.on_notification(n),
                        Message::Response(_) => {}
                    }
                }
                recv(events) -> ev => {
                    if let Ok(ev) = ev {
                        self.on_event(ev);
                    }
                }
                recv(timer) -> _ => self.flush_diagnostics(),
            }
        }
    }

    fn on_event(&mut self, ev: Event) {
        match ev {
            Event::Indexed(generation, files) => {
                if generation != self.scan_generation {
                    return;
                }
                log::info!("indexed {} files", files.len());
                self.session.index = Index::default();
                for f in files {
                    if !self.session.docs.contains_key(&f.uri) {
                        self.session.index.insert(f);
                    }
                }
                self.reindex_open_documents();
                self.publish_open_documents();
            }
        }
    }

    fn start_scan(&mut self) {
        self.scan_generation += 1;
        if !self.session.settings.config.workspace.index || self.session.roots.is_empty() {
            return;
        }
        let settings = self.session.settings.clone();
        let roots = self.session.roots.clone();
        let sender = self.events.0.clone();
        let generation = self.scan_generation;
        std::thread::spawn(move || {
            let files = workspace::scan(&settings, &roots);
            let _ = sender.send(Event::Indexed(generation, files));
        });
    }

    fn register_watchers(&mut self) {
        if !self.watch_files || !self.session.settings.config.workspace.index {
            return;
        }
        let exts: Vec<&str> = self
            .session
            .settings
            .dialects
            .iter()
            .flat_map(|d| d.extensions.iter().map(String::as_str))
            .collect();
        let options = DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![FileSystemWatcher {
                glob_pattern: GlobPattern::String(format!("**/*.{{{}}}", exts.join(","))),
                kind: None,
            }],
        };
        let params = RegistrationParams {
            registrations: vec![Registration {
                id: "llsp-watch".into(),
                method: notif::DidChangeWatchedFiles::METHOD.into(),
                register_options: serde_json::to_value(options).ok(),
            }],
        };
        self.next_request += 1;
        let id = RequestId::from(format!("llsp-{}", self.next_request));
        self.send(Request::new(id, req::RegisterCapability::METHOD.into(), params).into());
    }

    fn on_request(&mut self, req: Request) {
        let Request { id, method, params } = req;
        let resp = match method.as_str() {
            req::DocumentSymbolRequest::METHOD => {
                self.handle::<req::DocumentSymbolRequest>(id, params, symbols::document_symbols)
            }
            req::WorkspaceSymbolRequest::METHOD => {
                self.handle::<req::WorkspaceSymbolRequest>(id, params, symbols::workspace_symbols)
            }
            req::GotoDefinition::METHOD => {
                self.handle::<req::GotoDefinition>(id, params, navigation::definition)
            }
            req::References::METHOD => {
                self.handle::<req::References>(id, params, navigation::references)
            }
            req::DocumentHighlightRequest::METHOD => self.handle::<req::DocumentHighlightRequest>(
                id,
                params,
                navigation::document_highlight,
            ),
            req::PrepareRenameRequest::METHOD => {
                self.handle::<req::PrepareRenameRequest>(id, params, navigation::prepare_rename)
            }
            req::Rename::METHOD => self.handle::<req::Rename>(id, params, navigation::rename),
            req::Completion::METHOD => {
                self.handle::<req::Completion>(id, params, assist::completion)
            }
            req::SignatureHelpRequest::METHOD => {
                self.handle::<req::SignatureHelpRequest>(id, params, assist::signature_help)
            }
            req::HoverRequest::METHOD => {
                self.handle::<req::HoverRequest>(id, params, assist::hover)
            }
            req::Formatting::METHOD => {
                self.handle::<req::Formatting>(id, params, formatting::formatting)
            }
            req::RangeFormatting::METHOD => {
                self.handle::<req::RangeFormatting>(id, params, formatting::range_formatting)
            }
            req::FoldingRangeRequest::METHOD => {
                self.handle::<req::FoldingRangeRequest>(id, params, structure::folding_ranges)
            }
            req::SelectionRangeRequest::METHOD => {
                self.handle::<req::SelectionRangeRequest>(id, params, structure::selection_ranges)
            }
            req::SemanticTokensFullRequest::METHOD => self
                .handle::<req::SemanticTokensFullRequest>(
                    id,
                    params,
                    structure::semantic_tokens_full,
                ),
            req::SemanticTokensRangeRequest::METHOD => self
                .handle::<req::SemanticTokensRangeRequest>(
                    id,
                    params,
                    structure::semantic_tokens_range,
                ),
            _ => Response::new_err(
                id,
                ErrorCode::MethodNotFound as i32,
                format!("unknown method {method}"),
            ),
        };
        self.send(resp.into());
    }

    fn handle<R>(
        &mut self,
        id: RequestId,
        params: serde_json::Value,
        f: impl FnOnce(&Session, R::Params) -> FeatureResult<R::Result>,
    ) -> Response
    where
        R: req::Request,
        R::Params: DeserializeOwned,
    {
        let params = match serde_json::from_value::<R::Params>(params) {
            Ok(p) => p,
            Err(e) => {
                return Response::new_err(id, ErrorCode::InvalidParams as i32, e.to_string());
            }
        };
        // Request handlers only read state, so recovering is safe; sync notifications are
        // not wrapped because a panic mid-edit would leave the document silently wrong.
        match catch_unwind(AssertUnwindSafe(|| f(&self.session, params))) {
            Ok(Ok(result)) => Response::new_ok(id, result),
            Ok(Err(FeatureError::Invalid(message))) => {
                Response::new_err(id, ErrorCode::InvalidParams as i32, message)
            }
            Ok(Err(FeatureError::Failed(message))) => {
                Response::new_err(id, ErrorCode::RequestFailed as i32, message)
            }
            Err(panic) => {
                let message = format!("{} panicked: {}", R::METHOD, panic_message(&*panic));
                log::error!("{message}");
                Response::new_err(id, ErrorCode::InternalError as i32, message)
            }
        }
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
                self.notify_isolated::<notif::DidCloseTextDocument>(n, Self::did_close);
                Ok(())
            }
            notif::DidChangeConfiguration::METHOD => {
                self.notify_isolated::<notif::DidChangeConfiguration>(
                    n,
                    Self::did_change_configuration,
                );
                Ok(())
            }
            notif::DidChangeWatchedFiles::METHOD => {
                self.notify_isolated::<notif::DidChangeWatchedFiles>(
                    n,
                    Self::did_change_watched_files,
                );
                Ok(())
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

    /// For notifications that only touch derived state (index, settings, published
    /// diagnostics): a panic is logged and the server keeps running. `didOpen` and
    /// `didChange` stay on the unwrapped `notify` path, where a panic mid-edit would
    /// leave the document silently wrong.
    fn notify_isolated<N>(&mut self, n: Notification, f: impl FnOnce(&mut Self, N::Params))
    where
        N: notif::Notification,
        N::Params: DeserializeOwned,
    {
        let Ok(params) = serde_json::from_value::<N::Params>(n.params) else {
            log::warn!("{}: cannot decode params", N::METHOD);
            return;
        };
        match catch_unwind(AssertUnwindSafe(|| f(self, params))) {
            Ok(()) => {}
            Err(panic) => {
                log::error!("{} panicked: {}", N::METHOD, panic_message(&*panic));
            }
        }
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

    fn warn_oversized(&self, doc: &Document) {
        self.show_warning(format!(
            "llsp: {} is {} bytes, over files.max_file_size ({}); analysis disabled",
            doc.client_uri.as_str(),
            doc.text().len(),
            self.session.settings.config.files.max_file_size
        ));
    }

    fn did_open(&mut self, p: lsp_types::DidOpenTextDocumentParams) {
        let doc = p.text_document;
        let uri = normalize_uri(&doc.uri);
        let path = uri_to_path(&uri);
        let language_id = Some(doc.language_id);
        let dialect =
            self.session
                .settings
                .detect(path.as_deref(), language_id.as_deref(), &doc.text);
        let max = self.session.settings.config.files.max_file_size;
        let document = Document::new(doc.uri, doc.text, doc.version, language_id, dialect, max);
        if document.oversized() {
            self.warn_oversized(&document);
        }
        self.session.docs.insert(uri.clone(), document);
        self.document_changed(&uri);
    }

    fn did_change(&mut self, p: lsp_types::DidChangeTextDocumentParams) {
        let uri = normalize_uri(&p.text_document.uri);
        let Some(doc) = self.session.docs.get_mut(&uri) else {
            log::warn!("change for unopened document {}", uri.as_str());
            return;
        };
        let was_oversized = doc.oversized();
        doc.apply_changes(p.content_changes, p.text_document.version, self.session.enc);
        if doc.oversized() && !was_oversized {
            let doc = &self.session.docs[&uri];
            self.warn_oversized(doc);
        }
        self.document_changed(&uri);
    }

    fn did_close(&mut self, p: lsp_types::DidCloseTextDocumentParams) {
        let uri = normalize_uri(&p.text_document.uri);
        self.session.docs.remove(&uri);
        self.pending.remove(&uri);
        self.session.index.remove(&uri);
        if let Some(path) = uri_to_path(&uri)
            && workspace::is_inside(&self.session.roots, &path)
            && let Some(summary) = workspace::summarize(&self.session.settings, &path)
        {
            self.session.index.insert(summary);
        }
        self.send_notification::<notif::PublishDiagnostics>(PublishDiagnosticsParams {
            uri: p.text_document.uri,
            diagnostics: Vec::new(),
            version: None,
        });
    }

    fn did_change_watched_files(&mut self, p: lsp_types::DidChangeWatchedFilesParams) {
        for change in p.changes {
            let uri = normalize_uri(&change.uri);
            if self.session.docs.contains_key(&uri) {
                continue;
            }
            let Some(path) = uri_to_path(&uri) else {
                continue;
            };
            if change.typ == FileChangeType::DELETED {
                self.session.index.remove(&uri);
                continue;
            }
            if !workspace::is_inside(&self.session.roots, &path) {
                log::debug!("ignoring watched file outside roots: {}", path.display());
                continue;
            }
            match workspace::summarize(&self.session.settings, &path) {
                Some(s) => self.session.index.insert(s),
                None => self.session.index.remove(&uri),
            }
        }
    }

    fn did_change_configuration(&mut self, p: lsp_types::DidChangeConfigurationParams) {
        let settings = match p.settings {
            serde_json::Value::Object(mut o) if o.contains_key("llsp") => {
                o.remove("llsp").unwrap_or_default()
            }
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
                self.session.settings = settings;
                self.reload_documents();
                if !self.session.settings.config.workspace.index {
                    self.session.index = Index::default();
                }
                self.reindex_open_documents();
                self.publish_open_documents();
                self.start_scan();
            }
            Err(e) => self.show_warning(format!("llsp: configuration ignored: {e:#}")),
        }
    }

    fn reload_documents(&mut self) {
        let uris: Vec<Uri> = self.session.docs.keys().cloned().collect();
        for uri in uris {
            let doc = &self.session.docs[&uri];
            let path = uri_to_path(&uri);
            let dialect = self.session.settings.detect(
                path.as_deref(),
                doc.language_id.as_deref(),
                doc.text(),
            );
            let max = self.session.settings.config.files.max_file_size;
            let fresh = Document::new(
                doc.client_uri.clone(),
                doc.text().to_owned(),
                doc.version,
                doc.language_id.clone(),
                dialect,
                max,
            );
            self.session.docs.insert(uri, fresh);
        }
    }

    fn publish_open_documents(&self) {
        for uri in self.session.docs.keys() {
            self.publish_diagnostics(uri);
        }
    }

    fn reindex_open_documents(&mut self) {
        for (uri, doc) in &self.session.docs {
            self.session.index.insert(FileSummary::new(
                uri.clone(),
                doc.dialect.clone(),
                doc.tree(),
                doc.analysis(),
            ));
        }
    }

    fn document_changed(&mut self, uri: &Uri) {
        let Some(doc) = self.session.docs.get(uri) else {
            return;
        };
        self.session.index.insert(FileSummary::new(
            uri.clone(),
            doc.dialect.clone(),
            doc.tree(),
            doc.analysis(),
        ));
        match self.session.settings.config.diagnostics.debounce_ms {
            0 => self.publish_diagnostics(uri),
            ms => {
                self.pending
                    .insert(uri.clone(), Instant::now() + Duration::from_millis(ms));
            }
        }
    }

    fn flush_diagnostics(&mut self) {
        let now = Instant::now();
        let due: Vec<Uri> = self
            .pending
            .iter()
            .filter(|&(_, &at)| at <= now)
            .map(|(u, _)| u.clone())
            .collect();
        for uri in due {
            self.pending.remove(&uri);
            self.publish_diagnostics(&uri);
        }
    }

    fn publish_diagnostics(&self, uri: &Uri) {
        let Some(doc) = self.session.docs.get(uri) else {
            return;
        };
        let same = |f: &FileSummary| f.dialect.name == doc.dialect.name;
        let is_defined = |key: &str| {
            self.session
                .index
                .defs_named(key)
                .any(|(f, d)| same(f) && doc.dialect.cells_match(d.cell, Cell::Function))
        };
        let found = diagnostics::check(
            doc.tree(),
            doc.analysis(),
            &doc.dialect,
            &self.session.settings.config.diagnostics,
            is_defined,
        );
        let diagnostics = found
            .into_iter()
            .map(|d| lsp_types::Diagnostic {
                range: doc.range(d.start, d.end, self.session.enc),
                severity: Some(match d.severity {
                    Severity::Error => lsp_types::DiagnosticSeverity::ERROR,
                    Severity::Warning => lsp_types::DiagnosticSeverity::WARNING,
                    Severity::Info => lsp_types::DiagnosticSeverity::INFORMATION,
                    Severity::Hint => lsp_types::DiagnosticSeverity::HINT,
                }),
                code: Some(lsp_types::NumberOrString::String(d.code.into())),
                source: Some("llsp".into()),
                message: d.message,
                tags: d
                    .unnecessary
                    .then(|| vec![lsp_types::DiagnosticTag::UNNECESSARY]),
                ..Default::default()
            })
            .collect();
        self.send_notification::<notif::PublishDiagnostics>(PublishDiagnosticsParams {
            uri: doc.client_uri.clone(),
            diagnostics,
            version: Some(doc.version),
        });
    }
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::path_to_uri;

    fn server(root: &std::path::Path) -> (Server, Connection) {
        let (conn, client) = Connection::memory();
        let settings = Layers::default().resolve().unwrap();
        let s = Server {
            session: Session {
                settings,
                enc: Encoding::Utf16,
                roots: vec![root.canonicalize().unwrap()],
                docs: FxHashMap::default(),
                index: Index::default(),
            },
            conn,
            layers: Layers::default(),
            events: crossbeam_channel::unbounded(),
            scan_generation: 0,
            watch_files: false,
            next_request: 0,
            pending: FxHashMap::default(),
        };
        (s, client)
    }

    fn scan_now(s: &mut Server) {
        s.start_scan();
        let ev = s.events.1.recv().unwrap();
        s.on_event(ev);
    }

    fn open(s: &mut Server, uri: &Uri, text: &str) {
        s.did_open(lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: uri.clone(),
                language_id: "lisp".into(),
                version: 1,
                text: text.into(),
            },
        });
    }

    fn def_names(s: &Server, uri: &Uri) -> Vec<String> {
        s.session
            .index
            .get(uri)
            .unwrap()
            .defs
            .iter()
            .map(|d| d.name.clone())
            .collect()
    }

    #[test]
    fn open_document_overrides_disk_and_close_restores() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("a.lisp");
        std::fs::write(&path, "(defun old ())").unwrap();
        let (mut s, _client) = server(dir.path());
        scan_now(&mut s);
        let uri = path_to_uri(&path).unwrap();
        assert_eq!(def_names(&s, &uri), ["old"]);

        open(&mut s, &uri, "(defun old ()) (defun fresh ())");
        assert_eq!(def_names(&s, &uri), ["old", "fresh"]);

        s.did_close(lsp_types::DidCloseTextDocumentParams {
            text_document: lsp_types::TextDocumentIdentifier { uri: uri.clone() },
        });
        assert_eq!(def_names(&s, &uri), ["old"]);
    }

    #[test]
    fn scan_results_do_not_clobber_open_documents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().canonicalize().unwrap().join("a.lisp");
        std::fs::write(&path, "(defun disk ())").unwrap();
        let (mut s, _client) = server(dir.path());
        let uri = path_to_uri(&path).unwrap();
        open(&mut s, &uri, "(defun live ())");
        scan_now(&mut s);
        assert_eq!(def_names(&s, &uri), ["live"]);
    }

    #[test]
    fn watched_file_events() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (mut s, _client) = server(&root);
        let path = root.join("new.scm");
        std::fs::write(&path, "(define (f) 1)").unwrap();
        let outside = tempfile::tempdir().unwrap();
        let out_path = outside.path().canonicalize().unwrap().join("x.scm");
        std::fs::write(&out_path, "(define (secret) 1)").unwrap();
        let event = |uri: Uri, typ| lsp_types::FileEvent { uri, typ };
        let uri = path_to_uri(&path).unwrap();
        let out_uri = path_to_uri(&out_path).unwrap();
        s.did_change_watched_files(lsp_types::DidChangeWatchedFilesParams {
            changes: vec![
                event(uri.clone(), FileChangeType::CREATED),
                event(out_uri.clone(), FileChangeType::CREATED),
            ],
        });
        assert_eq!(def_names(&s, &uri), ["f"]);
        assert!(s.session.index.get(&out_uri).is_none());
        s.did_change_watched_files(lsp_types::DidChangeWatchedFilesParams {
            changes: vec![event(uri.clone(), FileChangeType::DELETED)],
        });
        assert!(s.session.index.is_empty());
    }

    #[test]
    fn config_change_keeps_index_until_rescan() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("a.lisp"), "(defun helper ())").unwrap();
        std::fs::write(root.join("gone.lisp"), "(defun gone ())").unwrap();
        let (mut s, _client) = server(&root);
        scan_now(&mut s);
        std::fs::remove_file(root.join("gone.lisp")).unwrap();

        s.did_change_configuration(lsp_types::DidChangeConfigurationParams {
            settings: serde_json::json!({"format": {"body_indent": 3}}),
        });
        assert_eq!(s.session.index.defs_named("helper").count(), 1);

        let ev = s.events.1.recv().unwrap();
        s.on_event(ev);
        assert_eq!(s.session.index.defs_named("helper").count(), 1);
        assert_eq!(
            s.session.index.defs_named("gone").count(),
            0,
            "rescan drops stale files"
        );
    }

    #[test]
    fn handler_panic_becomes_internal_error() {
        let dir = tempfile::tempdir().unwrap();
        let (mut s, _client) = server(dir.path());
        let params = serde_json::json!({
            "textDocument": {"uri": "file:///a.lisp"},
            "position": {"line": 0, "character": 0}
        });
        let resp = s.handle::<req::HoverRequest>(RequestId::from(1), params, |_, _| panic!("boom"));
        let err = resp.response_result.unwrap_err();
        assert_eq!(err.code, ErrorCode::InternalError as i32);
        assert!(err.message.contains("boom"), "{}", err.message);
    }

    #[test]
    fn watched_events_do_not_override_open_documents() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let path = root.join("a.lisp");
        std::fs::write(&path, "(defun disk ())").unwrap();
        let (mut s, _client) = server(&root);
        let uri = path_to_uri(&path).unwrap();
        open(&mut s, &uri, "(defun live ())");
        for typ in [FileChangeType::CHANGED, FileChangeType::DELETED] {
            s.did_change_watched_files(lsp_types::DidChangeWatchedFilesParams {
                changes: vec![lsp_types::FileEvent {
                    uri: uri.clone(),
                    typ,
                }],
            });
            assert_eq!(def_names(&s, &uri), ["live"]);
        }
    }

    #[test]
    fn indexing_can_be_disabled() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.lisp"), "(defun a ())").unwrap();
        let (mut s, _client) = server(dir.path());
        s.session.settings = Layers {
            cli: toml::from_str("[workspace]\nindex = false").unwrap(),
            ..Layers::default()
        }
        .resolve()
        .unwrap();
        s.start_scan();
        assert!(s.events.1.try_recv().is_err());
    }
}
