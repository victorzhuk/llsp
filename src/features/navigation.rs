use std::collections::HashMap;

use lsp_types::{
    DocumentHighlight, DocumentHighlightKind, DocumentHighlightParams, GotoDefinitionParams,
    GotoDefinitionResponse, Location, PrepareRenameResponse, ReferenceParams, RenameParams,
    TextDocumentPositionParams, TextEdit, Uri, WorkspaceEdit,
};

use super::FeatureError;
use super::FeatureResult;
use crate::analysis::{Target, symbol_like};
use crate::dialect::Cell;
use crate::document::Document;
use crate::session::{Session, Sym, owner, same_dialect};
use crate::syntax::{NodeKind, Tree};
use crate::workspace::{FileSummary, Ref};

pub(crate) fn definition(
    s: &Session,
    p: GotoDefinitionParams,
) -> FeatureResult<Option<GotoDefinitionResponse>> {
    let pos = p.text_document_position_params;
    let Some((uri, doc)) = s.document(&pos.text_document.uri) else {
        return Ok(None);
    };
    let Some((sym, _)) = s.symbol_at(doc, pos.position) else {
        return Ok(None);
    };
    let locations = match sym {
        Sym::Local(b) => {
            let b = &doc.analysis().binders[b as usize];
            vec![Location::new(
                doc.client_uri.clone(),
                doc.range(b.start, b.end, s.enc),
            )]
        }
        Sym::Global {
            key,
            explicit,
            cell,
            ..
        } => global_definitions(s, &uri, doc, &key, explicit.as_deref(), cell),
    };
    Ok((!locations.is_empty()).then_some(GotoDefinitionResponse::Array(locations)))
}

fn global_definitions(
    s: &Session,
    uri: &Uri,
    doc: &Document,
    key: &str,
    qualifier: Option<&str>,
    cell: Cell,
) -> Vec<Location> {
    let same = same_dialect(doc);
    let cells = &doc.dialect;
    let mut defs: Vec<_> = s
        .index
        .defs_named(key)
        .filter(|(f, d)| same(f) && cells.cells_match(d.cell, cell))
        .collect();
    if let Some(q) = qualifier
        && defs.iter().any(|(_, d)| d.namespace.as_deref() == Some(q))
    {
        defs.retain(|(_, d)| d.namespace.as_deref() == Some(q));
    }
    defs.sort_by_key(|(f, d)| (&f.uri != uri, f.uri.as_str().to_owned(), d.name_start));
    defs.into_iter()
        .map(|(f, d)| s.location(f, d.name_start, d.name_end))
        .collect()
}

pub(crate) fn references(s: &Session, p: ReferenceParams) -> FeatureResult<Option<Vec<Location>>> {
    let pos = p.text_document_position;
    let include_decl = p.context.include_declaration;
    let Some((_, doc)) = s.document(&pos.text_document.uri) else {
        return Ok(None);
    };
    let Some((sym, _)) = s.symbol_at(doc, pos.position) else {
        return Ok(None);
    };
    let locations = match sym {
        Sym::Local(b) => {
            let binder = &doc.analysis().binders[b as usize];
            local_occurrences(doc, b)
                .filter(|&(st, _)| include_decl || st != binder.start)
                .map(|(st, e)| Location::new(doc.client_uri.clone(), doc.range(st, e, s.enc)))
                .collect()
        }
        Sym::Global {
            key,
            explicit,
            context,
            cell,
        } => {
            let same = same_dialect(doc);
            let cells = &doc.dialect;
            let defined = s.defined_namespaces(doc, &key, cell);
            let target = owner(&defined, explicit.as_deref(), context.as_deref());
            let mut files: Vec<_> = s
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
                        .filter(|d| cells.cells_match(d.cell, cell))
                        .map(|d| (d.name_start, d.name_end))
                        .collect()
                };
                let loc = s.locator(f);
                out.extend(
                    f.refs[&key]
                        .iter()
                        .filter(|r| cells.cells_match(r.cell, cell))
                        .filter(|r| target.is_none() || resolves_to(&defined, f, r) == target)
                        .filter(|r| !decls.iter().any(|&(ds, de)| ds <= r.start && r.start < de))
                        .map(|r| loc(r.start, r.end)),
                );
            }
            out
        }
    };
    Ok(Some(locations))
}

pub(crate) fn document_highlight(
    s: &Session,
    p: DocumentHighlightParams,
) -> FeatureResult<Option<Vec<DocumentHighlight>>> {
    let pos = p.text_document_position_params;
    let Some((_, doc)) = s.document(&pos.text_document.uri) else {
        return Ok(None);
    };
    let Some((sym, _)) = s.symbol_at(doc, pos.position) else {
        return Ok(None);
    };
    let a = doc.analysis();
    let highlight = |st: u32, e: u32, write: bool| DocumentHighlight {
        range: doc.range(st, e, s.enc),
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
                .map(|(st, e)| highlight(st, e, st == start))
                .collect()
        }
        Sym::Global { key, cell, .. } => a
            .occurrences
            .iter()
            .filter(|o| {
                o.target == Target::Global && o.key == key && doc.dialect.cells_match(o.cell, cell)
            })
            .map(|o| {
                let is_def = a
                    .defs
                    .iter()
                    .any(|d| d.key == key && d.name_start <= o.start && o.start < d.name_end);
                highlight(o.start, o.end, is_def)
            })
            .collect(),
    };
    Ok(Some(out))
}

pub(crate) fn prepare_rename(
    s: &Session,
    p: TextDocumentPositionParams,
) -> FeatureResult<Option<PrepareRenameResponse>> {
    let Some((_, doc)) = s.document(&p.text_document.uri) else {
        return Ok(None);
    };
    let Some((sym, occ)) = s.symbol_at(doc, p.position) else {
        return Ok(None);
    };
    if let Sym::Global {
        key,
        explicit,
        context,
        cell,
    } = &sym
    {
        let defined = s.defined_namespaces(doc, key, *cell);
        rename_target(&defined, key, explicit.as_deref(), context.as_deref())?;
    }
    let text = &doc.text()[occ.start as usize..occ.end as usize];
    Ok(Some(PrepareRenameResponse::RangeWithPlaceholder {
        range: doc.range(occ.start, occ.end, s.enc),
        placeholder: text.to_owned(),
    }))
}

// lsp_types::Uri hashes and compares by its string; its lazily parsed parts never change that.
#[allow(clippy::mutable_key_type)]
pub(crate) fn rename(s: &Session, p: RenameParams) -> FeatureResult<Option<WorkspaceEdit>> {
    let pos = p.text_document_position;
    let Some((_, doc)) = s.document(&pos.text_document.uri) else {
        return Ok(None);
    };
    if !valid_symbol(doc, &p.new_name) {
        return Err(FeatureError::Invalid(format!(
            "{:?} is not a valid {} symbol",
            p.new_name, doc.dialect.name
        )));
    }
    let Some((sym, _)) = s.symbol_at(doc, pos.position) else {
        return Ok(None);
    };
    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    match sym {
        Sym::Local(b) => {
            let edits = local_occurrences(doc, b)
                .map(|(st, e)| TextEdit::new(doc.range(st, e, s.enc), p.new_name.clone()))
                .collect();
            changes.insert(doc.client_uri.clone(), edits);
        }
        Sym::Global {
            key,
            explicit,
            context,
            cell,
        } => {
            let same = same_dialect(doc);
            let cells = &doc.dialect;
            let defined = s.defined_namespaces(doc, &key, cell);
            let target = rename_target(&defined, &key, explicit.as_deref(), context.as_deref())?;
            let refs = s.index.refs_named(&key).filter(|&(f, r)| {
                same(f)
                    && cells.cells_match(r.cell, cell)
                    && resolves_to(&defined, f, r) == Some(target)
            });
            for (f, r) in refs {
                let loc = s.location(f, r.start, r.end);
                changes
                    .entry(loc.uri)
                    .or_default()
                    .push(TextEdit::new(loc.range, p.new_name.clone()));
            }
        }
    }
    Ok(Some(WorkspaceEdit::new(changes)))
}

fn resolves_to<'a>(
    defined: &[Option<&'a str>],
    file: &'a FileSummary,
    r: &'a Ref,
) -> Option<Option<&'a str>> {
    owner(defined, r.explicit.as_deref(), file.namespace_at(r.start))
}

/// The namespace whose definition a rename changes; an error when there is none or it is
/// ambiguous.
fn rename_target<'a>(
    defined: &[Option<&'a str>],
    key: &str,
    explicit: Option<&'a str>,
    context: Option<&'a str>,
) -> FeatureResult<Option<&'a str>> {
    match owner(defined, explicit, context) {
        Some(ns) if defined.contains(&ns) => Ok(ns),
        _ if defined.is_empty() || explicit.is_some() => Err(FeatureError::Failed(format!(
            "cannot rename {key}: no definition in the workspace"
        ))),
        _ => Err(FeatureError::Failed(format!(
            "cannot rename {key}: defined in several namespaces; qualify it or rename from its definition"
        ))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::Analysis;
    use crate::config::Layers;
    use crate::dialect::Dialects;
    use crate::syntax::Tree;
    use crate::workspace::Index;
    use lsp_types::{Position, TextDocumentIdentifier, TextDocumentPositionParams as TDPP};

    #[test]
    fn rename_target_policies() {
        // Unique defining namespace is the target.
        assert_eq!(
            rename_target(&[Some("app")], "run", None, None).unwrap(),
            Some("app")
        );
        // Several defining namespaces without a qualifier or context: refused.
        assert!(rename_target(&[Some("a"), Some("b")], "run", None, None).is_err());
        // Explicit qualifier that defines nothing: refused.
        assert!(rename_target(&[Some("a")], "run", Some("x"), None).is_err());
        // Nothing defines the name at all: refused.
        assert!(rename_target(&[], "run", None, None).is_err());
    }

    #[test]
    fn definition_resolves_through_session() {
        let d = Dialects::builtin().get("common-lisp").unwrap().clone();
        let source = "(defun helper () 1)\n(helper)";
        let tree = Tree::parse(source.to_owned(), &d);
        let analysis = Analysis::new(&tree, &d);
        let mut session = Session {
            settings: Layers::default().resolve().unwrap(),
            enc: crate::document::Encoding::Utf16,
            roots: vec![],
            docs: rustc_hash::FxHashMap::default(),
            index: Index::default(),
        };
        let uri: Uri = "file:///w/a.lisp".parse().unwrap();
        session
            .index
            .insert(FileSummary::new(uri.clone(), d.clone(), &tree, &analysis));
        // Definition is served from the open document's live analysis.
        session.docs.insert(
            crate::document::normalize_uri(&uri),
            Document::new(uri.clone(), source.to_owned(), 1, None, d, u64::MAX),
        );

        let p = GotoDefinitionParams {
            text_document_position_params: TDPP {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 1,
                    character: 1,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let r = definition(&session, p).unwrap().unwrap();
        let GotoDefinitionResponse::Array(locs) = r else {
            panic!("expected array");
        };
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].range.start.line, 0);
    }
}
