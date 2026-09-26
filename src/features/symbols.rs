use lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, OneOf, WorkspaceSymbol,
    WorkspaceSymbolParams, WorkspaceSymbolResponse,
};

use super::lsp_symbol_kind;
use crate::server::{HandlerResult, Server};

impl Server {
    pub(crate) fn document_symbols(
        &mut self,
        p: DocumentSymbolParams,
    ) -> HandlerResult<Option<DocumentSymbolResponse>> {
        let Some((_, doc)) = self.document(&p.text_document.uri) else {
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
                detail: d.signatures.first().map(|s| s.label.clone()),
                kind: lsp_symbol_kind(d.kind),
                tags: None,
                deprecated: None,
                range: doc.range(d.start, d.end, self.enc),
                selection_range: doc.range(d.name_start, d.name_end, self.enc),
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
        &mut self,
        p: WorkspaceSymbolParams,
    ) -> HandlerResult<Option<WorkspaceSymbolResponse>> {
        let query = p.query.to_lowercase();
        let mut hits: Vec<_> = self
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
        hits.truncate(self.settings.config.workspace.max_symbols);
        let symbols = hits
            .into_iter()
            .map(|(_, f, d)| WorkspaceSymbol {
                name: d.name.clone(),
                kind: lsp_symbol_kind(d.kind),
                tags: None,
                container_name: d.namespace.clone(),
                location: OneOf::Left(self.location(f, d.name_start, d.name_end)),
                data: None,
            })
            .collect();
        Ok(Some(WorkspaceSymbolResponse::Nested(symbols)))
    }
}

/// Lower is better: 0 exact, 1 prefix, 2 substring, 3 subsequence; `None` for no match.
/// `query` must already be lowercase.
pub(crate) fn fuzzy_score(query: &str, name: &str) -> Option<u8> {
    if name.bytes().any(|b| b.is_ascii_uppercase()) || !name.is_ascii() {
        return score_lowercase(query, &name.to_lowercase());
    }
    score_lowercase(query, name)
}

/// Same as [`fuzzy_score`] for a name that is already lowercase.
pub(crate) fn score_lowercase(query: &str, name: &str) -> Option<u8> {
    if !is_subsequence(query, name) {
        return None;
    }
    Some(if query.is_empty() {
        3
    } else if name == query {
        0
    } else if name.starts_with(query) {
        1
    } else if name.contains(query) {
        2
    } else {
        3
    })
}

fn is_subsequence(query: &str, name: &str) -> bool {
    let mut want = query.as_bytes().iter().peekable();
    for b in name.bytes() {
        match want.peek() {
            Some(&&q) if q == b => {
                want.next();
            }
            Some(_) => {}
            None => break,
        }
    }
    want.peek().is_none()
}

#[cfg(test)]
mod tests {
    use super::fuzzy_score;

    #[test]
    fn fuzzy_ranks() {
        assert_eq!(fuzzy_score("point", "point"), Some(0));
        assert_eq!(fuzzy_score("po", "Point-X"), Some(1));
        assert_eq!(fuzzy_score("int", "point-x"), Some(2));
        assert_eq!(fuzzy_score("mkp", "make-point"), Some(3));
        assert_eq!(fuzzy_score("mkp", "mapcar-safe"), None);
        assert_eq!(fuzzy_score("", "x"), Some(3));
        assert_eq!(fuzzy_score("ö", "Öl"), Some(1));
        assert_eq!(fuzzy_score("öx", "Ölx"), Some(3));
    }
}
