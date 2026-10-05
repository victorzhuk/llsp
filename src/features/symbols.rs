use lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, OneOf, WorkspaceSymbol,
    WorkspaceSymbolParams, WorkspaceSymbolResponse,
};

use super::FeatureResult;
use super::lsp_symbol_kind;
use crate::session::Session;

pub(crate) fn document_symbols(
    s: &Session,
    p: DocumentSymbolParams,
) -> FeatureResult<Option<DocumentSymbolResponse>> {
    let Some((_, doc)) = s.document(&p.text_document.uri) else {
        return Ok(None);
    };
    let mut defs: Vec<_> = doc.analysis().defs.iter().collect();
    defs.sort_by_key(|d| (d.start, std::cmp::Reverse(d.end)));

    let mut roots: Vec<DocumentSymbol> = Vec::new();
    let mut stack: Vec<(u32, DocumentSymbol)> = Vec::new();
    let pop = |stack: &mut Vec<(u32, DocumentSymbol)>, roots: &mut Vec<DocumentSymbol>| {
        let (_, sym) = stack.pop().expect("non-empty");
        match stack.last_mut() {
            Some((_, parent)) => parent.children.get_or_insert_with(Vec::new).push(sym),
            None => roots.push(sym),
        }
    };
    for d in defs {
        while stack.last().is_some_and(|(end, _)| d.start >= *end) {
            pop(&mut stack, &mut roots);
        }
        #[allow(deprecated)]
        let sym = DocumentSymbol {
            name: d.name.clone(),
            detail: d.signatures.first().map(|sig| sig.label.clone()),
            kind: lsp_symbol_kind(d.kind),
            tags: None,
            deprecated: None,
            range: doc.range(d.start, d.end, s.enc),
            selection_range: doc.range(d.name_start, d.name_end, s.enc),
            children: None,
        };
        stack.push((d.end, sym));
    }
    while !stack.is_empty() {
        pop(&mut stack, &mut roots);
    }
    Ok(Some(DocumentSymbolResponse::Nested(roots)))
}

pub(crate) fn workspace_symbols(
    s: &Session,
    p: WorkspaceSymbolParams,
) -> FeatureResult<Option<WorkspaceSymbolResponse>> {
    let query = p.query.to_lowercase();
    let mut hits: Vec<_> = s
        .index
        .files()
        .flat_map(|f| f.search(&query).map(move |(score, d)| (score, f, d)))
        .collect();
    hits.sort_by(|a, b| {
        (a.0, a.2.name.len(), &a.2.name, a.1.uri.as_str()).cmp(&(
            b.0,
            b.2.name.len(),
            &b.2.name,
            b.1.uri.as_str(),
        ))
    });
    hits.truncate(s.settings.config.workspace.max_symbols);
    let symbols = hits
        .into_iter()
        .map(|(_, f, d)| WorkspaceSymbol {
            name: d.name.clone(),
            kind: lsp_symbol_kind(d.kind),
            tags: None,
            container_name: d.namespace.clone(),
            location: OneOf::Left(s.location(f, d.name_start, d.name_end)),
            data: None,
        })
        .collect();
    Ok(Some(WorkspaceSymbolResponse::Nested(symbols)))
}
