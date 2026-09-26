use std::collections::HashMap;

use lsp_types::{
    DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams, GotoDefinitionParams,
    GotoDefinitionResponse, Location, PrepareRenameResponse, ReferenceParams, RenameParams,
    TextDocumentPositionParams, TextEdit, Uri, WorkspaceEdit,
};

use super::Sym;
use crate::analysis::{Target, symbol_like};
use crate::document::Document;
use crate::server::{HandlerResult, ResponseError, Server};
use crate::syntax::{NodeKind, Tree};

impl Server {
    pub(crate) fn definition(
        &mut self,
        p: GotoDefinitionParams,
    ) -> HandlerResult<Option<GotoDefinitionResponse>> {
        let pos = p.text_document_position_params;
        let Some((uri, doc)) = self.document(&pos.text_document.uri) else {
            return Ok(None);
        };
        let Some((sym, _)) = self.symbol_at(doc, pos.position) else {
            return Ok(None);
        };
        let locations = match sym {
            Sym::Local(b) => {
                let b = &doc.analysis().binders[b as usize];
                vec![Location::new(
                    doc.client_uri.clone(),
                    doc.range(b.start, b.end, self.enc),
                )]
            }
            Sym::Global { key, qualifier } => {
                self.global_definitions(&uri, doc, &key, qualifier.as_deref())
            }
        };
        Ok((!locations.is_empty()).then_some(GotoDefinitionResponse::Array(locations)))
    }

    fn global_definitions(
        &self,
        uri: &Uri,
        doc: &Document,
        key: &str,
        qualifier: Option<&str>,
    ) -> Vec<Location> {
        let same = self.same_dialect(doc);
        let mut defs: Vec<_> = self
            .index
            .defs_named(key)
            .filter(|(f, _)| same(f))
            .collect();
        if let Some(q) = qualifier
            && defs.iter().any(|(_, d)| d.namespace.as_deref() == Some(q))
        {
            defs.retain(|(_, d)| d.namespace.as_deref() == Some(q));
        }
        defs.sort_by_key(|(f, d)| (&f.uri != uri, f.uri.as_str().to_owned(), d.name_start));
        defs.into_iter()
            .map(|(f, d)| self.location(f, d.name_start, d.name_end))
            .collect()
    }

    pub(crate) fn references(
        &mut self,
        p: ReferenceParams,
    ) -> HandlerResult<Option<Vec<Location>>> {
        let pos = p.text_document_position;
        let include_decl = p.context.include_declaration;
        let Some((_, doc)) = self.document(&pos.text_document.uri) else {
            return Ok(None);
        };
        let Some((sym, _)) = self.symbol_at(doc, pos.position) else {
            return Ok(None);
        };
        let locations = match sym {
            Sym::Local(b) => {
                let binder = &doc.analysis().binders[b as usize];
                local_occurrences(doc, b)
                    .filter(|&(s, _)| include_decl || s != binder.start)
                    .map(|(s, e)| Location::new(doc.client_uri.clone(), doc.range(s, e, self.enc)))
                    .collect()
            }
            Sym::Global { key, .. } => {
                let same = self.same_dialect(doc);
                let mut files: Vec<_> = self
                    .index
                    .files()
                    .filter(|f| same(f) && f.refs.contains_key(&key))
                    .collect();
                files.sort_by(|a, b| a.uri.as_str().cmp(b.uri.as_str()));
                let mut out = Vec::new();
                for f in files {
                    let decls: Vec<(u32, u32)> = if include_decl {
                        Vec::new()
                    } else {
                        f.defs_named(&key)
                            .map(|d| (d.name_start, d.name_end))
                            .collect()
                    };
                    let loc = self.locator(f);
                    out.extend(
                        f.refs[&key]
                            .iter()
                            .filter(|(s, _)| !decls.iter().any(|&(ds, de)| ds <= *s && *s < de))
                            .map(|&(s, e)| loc(s, e)),
                    );
                }
                out
            }
        };
        Ok(Some(locations))
    }

    pub(crate) fn document_highlight(
        &mut self,
        p: DocumentHighlightParams,
    ) -> HandlerResult<Option<Vec<DocumentHighlight>>> {
        let pos = p.text_document_position_params;
        let Some((_, doc)) = self.document(&pos.text_document.uri) else {
            return Ok(None);
        };
        let Some((sym, _)) = self.symbol_at(doc, pos.position) else {
            return Ok(None);
        };
        let a = doc.analysis();
        let highlight = |s: u32, e: u32, write: bool| DocumentHighlight {
            range: doc.range(s, e, self.enc),
            kind: Some(if write {
                DocumentHighlightKind::WRITE
            } else {
                DocumentHighlightKind::READ
            }),
        };
        let out = match sym {
            Sym::Local(b) => {
                let start = a.binders[b as usize].start;
                local_occurrences(doc, b)
                    .map(|(s, e)| highlight(s, e, s == start))
                    .collect()
            }
            Sym::Global { key, .. } => {
                a.occurrences
                    .iter()
                    .filter(|o| o.target == Target::Global && o.key == key)
                    .map(|o| {
                        let is_def = a.defs.iter().any(|d| {
                            d.key == key && d.name_start <= o.start && o.start < d.name_end
                        });
                        highlight(o.start, o.end, is_def)
                    })
                    .collect()
            }
        };
        Ok(Some(out))
    }

    pub(crate) fn prepare_rename(
        &mut self,
        p: TextDocumentPositionParams,
    ) -> HandlerResult<Option<PrepareRenameResponse>> {
        let Some((_, doc)) = self.document(&p.text_document.uri) else {
            return Ok(None);
        };
        let Some((sym, occ)) = self.symbol_at(doc, p.position) else {
            return Ok(None);
        };
        if let Sym::Global { key, .. } = &sym {
            let same = self.same_dialect(doc);
            if !self.index.defs_named(key).any(|(f, _)| same(f)) {
                return Err(ResponseError::request_failed(format!(
                    "cannot rename {key}: no definition in the workspace"
                )));
            }
        }
        let text = &doc.text()[occ.start as usize..occ.end as usize];
        Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: doc.range(occ.start, occ.end, self.enc),
            placeholder: text.to_owned(),
        }))
    }

    // lsp_types::Uri hashes and compares by its string; its lazily parsed parts never change that.
    #[allow(clippy::mutable_key_type)]
    pub(crate) fn rename(&mut self, p: RenameParams) -> HandlerResult<Option<WorkspaceEdit>> {
        let pos = p.text_document_position;
        let Some((_, doc)) = self.document(&pos.text_document.uri) else {
            return Ok(None);
        };
        if !valid_symbol(doc, &p.new_name) {
            return Err(ResponseError::invalid_params(format!(
                "{:?} is not a valid {} symbol",
                p.new_name, doc.dialect.name
            )));
        }
        let Some((sym, _)) = self.symbol_at(doc, pos.position) else {
            return Ok(None);
        };
        let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
        match sym {
            Sym::Local(b) => {
                let edits = local_occurrences(doc, b)
                    .map(|(s, e)| TextEdit::new(doc.range(s, e, self.enc), p.new_name.clone()))
                    .collect();
                changes.insert(doc.client_uri.clone(), edits);
            }
            Sym::Global { key, .. } => {
                let same = self.same_dialect(doc);
                if !self.index.defs_named(&key).any(|(f, _)| same(f)) {
                    return Err(ResponseError::request_failed(format!(
                        "cannot rename {key}: no definition in the workspace"
                    )));
                }
                for (f, (s, e)) in self.index.refs_named(&key).filter(|(f, _)| same(f)) {
                    let loc = self.location(f, s, e);
                    changes
                        .entry(loc.uri)
                        .or_default()
                        .push(TextEdit::new(loc.range, p.new_name.clone()));
                }
            }
        }
        Ok(Some(WorkspaceEdit::new(changes)))
    }
}

fn local_occurrences(doc: &Document, binder: u32) -> impl Iterator<Item = (u32, u32)> + '_ {
    doc.analysis()
        .occurrences
        .iter()
        .filter(move |o| o.target == Target::Local(binder))
        .map(|o| (o.start, o.end))
}

fn valid_symbol(doc: &Document, name: &str) -> bool {
    let tree = Tree::parse(name.to_owned(), &doc.dialect);
    tree.errors().is_empty()
        && tree.children(Tree::ROOT).len() == 1
        && tree.node(tree.children(Tree::ROOT)[0]).kind == NodeKind::Atom
        && tree.tokens().len() == 1
        && symbol_like(&doc.dialect, name)
        && doc.dialect.split_qualified(name).0.is_none()
}
