use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionList, CompletionParams, CompletionResponse,
    CompletionTextEdit, Documentation, Hover, HoverContents, HoverParams, MarkupContent,
    MarkupKind, ParameterInformation, ParameterLabel, SignatureHelp, SignatureHelpParams,
    SignatureInformation, TextEdit,
};
use rustc_hash::FxHashSet;

use super::{Sym, symbols::fuzzy_score};
use crate::analysis::{Analysis, Def, Signature};
use crate::dialect::{Cell, Dialect, SymbolKind};
use crate::document::Document;
use crate::server::{HandlerResult, Server};
use crate::syntax::{Delim, NodeId, NodeKind, Tree};
use crate::workspace::FileSummary;

struct Candidate {
    label: String,
    kind: CompletionItemKind,
    detail: Option<String>,
    doc: Option<String>,
    score: u8,
    rank: u8,
}

impl Server {
    pub(crate) fn completion(
        &mut self,
        p: CompletionParams,
    ) -> HandlerResult<Option<CompletionResponse>> {
        let pos = p.text_document_position;
        let Some((_, doc)) = self.document(&pos.text_document.uri) else {
            return Ok(None);
        };
        let offset = doc.offset(pos.position, self.enc);
        let tree = doc.tree();
        if tree.in_literal_or_comment(offset) {
            return Ok(None);
        }
        let typed = match tree.atom_at(offset) {
            Some(atom) => &doc.text()[tree.node(atom).start as usize..offset as usize],
            None => "",
        };
        if doc.dialect.is_keyword(typed) || typed.starts_with('#') {
            return Ok(None);
        }
        let (qualifier, base) = doc.dialect.split_qualified(typed);
        let base_start = offset - base.len() as u32;
        let query = base.to_lowercase();
        let a = doc.analysis();
        let same = self.same_dialect(doc);
        let want_cell = completion_cell(tree, &doc.dialect, a, offset);
        let cfg = &self.settings.config.completion;

        let mut out: Vec<Candidate> = Vec::new();
        let mut push = |c: Candidate| out.push(c);
        match qualifier {
            Some(q) => {
                let ns = doc.dialect.normalize(a.resolve_qualifier(q)).into_owned();
                for f in self.index.files().filter(|f| same(f)) {
                    for d in f.defs.iter().filter(|d| {
                        d.namespace.as_deref() == Some(&ns)
                            && doc.dialect.cells_match(d.cell, want_cell)
                    }) {
                        if let Some(score) = fuzzy_score(&query, &d.name) {
                            push(def_candidate(d, score));
                        }
                    }
                }
            }
            None => {
                if want_cell == Cell::Value {
                    for b in a.visible_binders(offset) {
                        if let Some(score) = fuzzy_score(&query, &b.name)
                            && !(b.start <= offset && offset <= b.end)
                        {
                            push(Candidate {
                                label: b.name.clone(),
                                kind: CompletionItemKind::VARIABLE,
                                detail: Some("local".into()),
                                doc: None,
                                score,
                                rank: 0,
                            });
                        }
                    }
                }
                for f in self.index.files().filter(|f| same(f)) {
                    for (score, d) in f
                        .search(&query)
                        .filter(|(_, d)| doc.dialect.cells_match(d.cell, want_cell))
                    {
                        push(def_candidate(d, score));
                    }
                }
                if cfg.builtins {
                    let d = &doc.dialect;
                    let lists = [
                        (
                            &d.special_forms,
                            CompletionItemKind::KEYWORD,
                            "special form",
                        ),
                        (&d.builtins, CompletionItemKind::FUNCTION, "builtin"),
                        (&d.constants, CompletionItemKind::CONSTANT, "constant"),
                    ];
                    for (names, kind, detail) in lists {
                        for n in names {
                            if let Some(score) = fuzzy_score(&query, n) {
                                push(Candidate {
                                    label: n.clone(),
                                    kind,
                                    detail: Some(detail.into()),
                                    doc: None,
                                    score,
                                    rank: 2,
                                });
                            }
                        }
                    }
                }
            }
        }

        out.sort_by(|a, b| {
            (a.score, a.rank, a.label.len(), &a.label).cmp(&(
                b.score,
                b.rank,
                b.label.len(),
                &b.label,
            ))
        });
        let mut seen = FxHashSet::default();
        out.retain(|c| seen.insert(doc.dialect.normalize(&c.label).into_owned()));
        let incomplete = out.len() > cfg.max_items;
        out.truncate(cfg.max_items);

        let range = doc.range(base_start, offset, self.enc);
        let items = out
            .into_iter()
            .enumerate()
            .map(|(i, c)| CompletionItem {
                label: c.label.clone(),
                kind: Some(c.kind),
                detail: c.detail,
                documentation: c.doc.map(|d| {
                    Documentation::MarkupContent(MarkupContent {
                        kind: MarkupKind::Markdown,
                        value: d,
                    })
                }),
                sort_text: Some(format!("{i:05}")),
                filter_text: Some(c.label.clone()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(range, c.label))),
                ..Default::default()
            })
            .collect();
        Ok(Some(CompletionResponse::List(CompletionList {
            is_incomplete: incomplete,
            items,
        })))
    }

    pub(crate) fn signature_help(
        &mut self,
        p: SignatureHelpParams,
    ) -> HandlerResult<Option<SignatureHelp>> {
        let pos = p.text_document_position_params;
        let Some((uri, doc)) = self.document(&pos.text_document.uri) else {
            return Ok(None);
        };
        let offset = doc.offset(pos.position, self.enc);
        let Some((call, head)) = enclosing_call(doc, offset) else {
            return Ok(None);
        };
        let tree = doc.tree();
        let head_node = tree.node(head);
        let a = doc.analysis();
        let Some(occ) = a.occurrence_at(head_node.end) else {
            return Ok(None);
        };
        if occ.target != crate::analysis::Target::Global {
            return Ok(None);
        }
        let same = self.same_dialect(doc);
        let mut defs: Vec<(&std::sync::Arc<FileSummary>, &Def)> = self
            .index
            .defs_named(&occ.key)
            .filter(|(f, d)| same(f) && doc.dialect.cells_match(d.cell, Cell::Function))
            .collect();
        defs.sort_by_key(|(f, _)| f.uri != uri);
        let name = tree.node_text(head);
        let sigs: Vec<&Signature> = defs.iter().flat_map(|(_, d)| &d.signatures).collect();
        if sigs.is_empty() {
            return Ok(None);
        }
        let args = tree
            .children(call)
            .iter()
            .skip(1)
            .filter(|&&c| tree.node(c).end < offset)
            .count();
        let active_sig = sigs
            .iter()
            .position(|s| {
                let (positional, rest) = param_slots(&doc.dialect, s);
                args < positional.len() || rest.is_some()
            })
            .unwrap_or(0);
        let docs = |i: usize| -> Option<String> {
            let mut n = 0;
            for (_, d) in &defs {
                n += d.signatures.len();
                if i < n {
                    return d.doc.clone();
                }
            }
            None
        };
        let signatures = sigs
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let (label, offsets) = signature_label(name, s);
                let (positional, rest) = param_slots(&doc.dialect, s);
                let active = positional
                    .get(args)
                    .copied()
                    .or(rest)
                    .or(positional.last().copied());
                SignatureInformation {
                    label,
                    documentation: docs(i).map(|d| {
                        Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: d,
                        })
                    }),
                    parameters: Some(
                        offsets
                            .into_iter()
                            .map(|(s, e)| ParameterInformation {
                                label: ParameterLabel::LabelOffsets([s, e]),
                                documentation: None,
                            })
                            .collect(),
                    ),
                    active_parameter: active.map(|a| a as u32),
                }
            })
            .collect();
        let active_parameter = {
            let (positional, rest) = param_slots(&doc.dialect, sigs[active_sig]);
            positional
                .get(args)
                .copied()
                .or(rest)
                .or(positional.last().copied())
        };
        Ok(Some(SignatureHelp {
            signatures,
            active_signature: Some(active_sig as u32),
            active_parameter: active_parameter.map(|a| a as u32),
        }))
    }

    pub(crate) fn hover(&mut self, p: HoverParams) -> HandlerResult<Option<Hover>> {
        let pos = p.text_document_position_params;
        let Some((uri, doc)) = self.document(&pos.text_document.uri) else {
            return Ok(None);
        };
        let Some((sym, occ)) = self.symbol_at(doc, pos.position) else {
            return Ok(None);
        };
        let lang = doc
            .dialect
            .language_ids
            .first()
            .map_or(doc.dialect.name.as_str(), String::as_str);
        let text = match sym {
            Sym::Local(b) => {
                let b = &doc.analysis().binders[b as usize];
                format!("```{lang}\n{}\n```\nlocal binding", b.name)
            }
            Sym::Global {
                key,
                explicit,
                cell,
                ..
            } => {
                let same = self.same_dialect(doc);
                let mut defs: Vec<_> = self
                    .index
                    .defs_named(&key)
                    .filter(|(f, d)| same(f) && doc.dialect.cells_match(d.cell, cell))
                    .collect();
                if let Some(q) = &explicit
                    && defs.iter().any(|(_, d)| d.namespace.as_ref() == Some(q))
                {
                    defs.retain(|(_, d)| d.namespace.as_ref() == Some(q));
                }
                defs.sort_by_key(|(f, d)| (f.uri != uri, d.start));
                if defs.is_empty() {
                    let d = &doc.dialect;
                    let category = if d.is_special_form(&key) {
                        "special form"
                    } else if d.is_builtin(&key) {
                        "builtin"
                    } else {
                        return Ok(None);
                    };
                    format!(
                        "```{lang}\n{}\n```\n{category} ({})",
                        occ_text(doc, occ.start, occ.end),
                        d.name
                    )
                } else {
                    defs.iter()
                        .take(3)
                        .map(|(f, d)| describe(lang, f, d))
                        .collect::<Vec<_>>()
                        .join("\n\n---\n\n")
                }
            }
        };
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: text,
            }),
            range: Some(doc.range(occ.start, occ.end, self.enc)),
        }))
    }
}

/// Requested cell for completion at `offset`: the analysis occurrence's cell for an
/// existing atom, otherwise the cell an atom completed at `offset` would get.
pub(crate) fn completion_cell(tree: &Tree, d: &Dialect, a: &Analysis, offset: u32) -> Cell {
    if !d.function_cells {
        return Cell::Value;
    }
    if let Some(atom) = tree.atom_at(offset)
        && let Some(o) = a.occurrences.iter().find(|o| o.node == atom)
    {
        return o.cell;
    }
    let id = tree.node_at(offset);
    match tree.node(id).kind {
        NodeKind::Prefix => {
            if tree.prefix_text(id) == "#'" {
                Cell::Function
            } else {
                Cell::Value
            }
        }
        NodeKind::List(Delim::Paren) => {
            let before = tree
                .children(id)
                .iter()
                .filter(|&&c| tree.node(c).end <= offset)
                .count();
            match before {
                0 if !gap_value_cell(tree, d, id) => Cell::Function,
                1 if tree.head(id).is_some_and(|h| d.normalize(h) == "function") => Cell::Function,
                _ => tree
                    .head(id)
                    .and_then(|h| d.def_spec(&d.normalize(h)))
                    .filter(|s| s.name == before)
                    .map_or(Cell::Value, |s| s.cell),
            }
        }
        _ => Cell::Value,
    }
}

/// True when a new head of the list `id` would sit in a value position: inside a
/// `cond` clause test or a reader vector's data.
fn gap_value_cell(tree: &Tree, d: &Dialect, id: NodeId) -> bool {
    let Some(gp) = tree.parent(id) else {
        return false;
    };
    match tree.node(gp).kind {
        NodeKind::Prefix => tree.prefix_text(gp) == "#",
        NodeKind::List(Delim::Paren) => tree.head(gp).is_some_and(|h| d.normalize(h) == "cond"),
        _ => false,
    }
}

fn def_candidate(d: &Def, score: u8) -> Candidate {
    Candidate {
        label: d.name.clone(),
        kind: match d.kind {
            SymbolKind::Function | SymbolKind::Test => CompletionItemKind::FUNCTION,
            SymbolKind::Macro => CompletionItemKind::KEYWORD,
            SymbolKind::Variable => CompletionItemKind::VARIABLE,
            SymbolKind::Constant => CompletionItemKind::CONSTANT,
            SymbolKind::Class | SymbolKind::Type => CompletionItemKind::CLASS,
            SymbolKind::Struct => CompletionItemKind::STRUCT,
            SymbolKind::Interface => CompletionItemKind::INTERFACE,
            SymbolKind::Method => CompletionItemKind::METHOD,
            SymbolKind::Module => CompletionItemKind::MODULE,
        },
        detail: d.signatures.first().map(|s| s.label.clone()),
        doc: d.doc.clone(),
        score,
        rank: 1,
    }
}

fn occ_text(doc: &Document, start: u32, end: u32) -> &str {
    &doc.text()[start as usize..end as usize]
}

fn describe(lang: &str, f: &FileSummary, d: &Def) -> String {
    let mut out = String::new();
    out.push_str(&format!("```{lang}\n"));
    if d.signatures.is_empty() {
        out.push_str(&d.name);
    } else {
        let sigs: Vec<_> = d
            .signatures
            .iter()
            .map(|s| signature_label(&d.name, s).0)
            .collect();
        out.push_str(&sigs.join("\n"));
    }
    out.push_str("\n```\n");
    let kind = format!("{:?}", d.kind).to_lowercase();
    let file = f.uri.as_str().rsplit('/').next().unwrap_or_default();
    let line = f.lines.line_col(d.start.into()).line + 1;
    match &d.namespace {
        Some(ns) => out.push_str(&format!("{kind} in `{ns}` · {file}:{line}")),
        None => out.push_str(&format!("{kind} · {file}:{line}")),
    }
    if let Some(doc) = &d.doc {
        out.push_str("\n\n");
        out.push_str(doc);
    }
    out
}

/// `(name p1 p2)` plus the byte offsets of each parameter within it.
fn signature_label(name: &str, s: &Signature) -> (String, Vec<(u32, u32)>) {
    let mut label = format!("({name}");
    let mut offsets = Vec::with_capacity(s.params.len());
    for p in &s.params {
        label.push(' ');
        let start = label.encode_utf16().count() as u32;
        label.push_str(p);
        offsets.push((start, label.encode_utf16().count() as u32));
    }
    label.push(')');
    (label, offsets)
}

/// Indices of positional parameters, and of the rest parameter if any.
fn param_slots(d: &Dialect, s: &Signature) -> (Vec<usize>, Option<usize>) {
    let mut positional = Vec::new();
    let mut rest = None;
    let mut in_rest = false;
    let mut keys = false;
    for (i, p) in s.params.iter().enumerate() {
        let key = d.normalize(p);
        match key.as_ref() {
            "&rest" | "&body" | "&" | "." | "&more" => in_rest = true,
            "&key" => keys = true,
            _ if key.starts_with('&') || d.is_pattern_ignored(&key) => {}
            _ if in_rest => {
                rest.get_or_insert(i);
            }
            _ if !keys => positional.push(i),
            _ => {}
        }
    }
    (positional, rest)
}

/// Innermost parenthesized call around `offset` whose head is a symbol, and that head.
fn enclosing_call(doc: &Document, offset: u32) -> Option<(NodeId, NodeId)> {
    let tree = doc.tree();
    let start = tree.node_at(offset);
    for id in tree.ancestors(start) {
        let n = tree.node(id);
        if n.kind != NodeKind::List(Delim::Paren)
            || offset <= n.start
            || (n.closed && offset >= n.end)
        {
            continue;
        }
        let Some(head) = tree.child(id, 0) else {
            continue;
        };
        let h = tree.node(head);
        if h.kind != NodeKind::Atom || (h.start <= offset && offset <= h.end) {
            continue;
        }
        return Some((id, head));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::completion_cell;
    use crate::analysis::Analysis;
    use crate::dialect::Cell;
    use crate::dialect::Dialects;
    use crate::syntax::Tree;

    fn cell(dialect: &str, src: &str, offset: u32) -> Cell {
        let ds = Dialects::builtin();
        let d = ds.get(dialect).unwrap();
        let t = Tree::parse(src.to_owned(), d);
        let a = Analysis::new(&t, d);
        completion_cell(&t, d, &a, offset)
    }

    #[test]
    fn function_reference_positions_are_function() {
        assert_eq!(cell("lispico-cl", "(function noise)", 12), Cell::Function);
        assert_eq!(cell("lispico-cl", "#'noise", 5), Cell::Function);
        assert_eq!(cell("lispico-cl", "#'", 2), Cell::Function);
        assert_eq!(cell("lispico-cl", "(function )", 10), Cell::Function);
    }

    #[test]
    fn cond_tests_and_reader_vectors_are_value() {
        assert_eq!(cell("lispico-cl", "(cond (noise 1))", 9), Cell::Value);
        assert_eq!(cell("lispico-cl", "#(noise 1)", 4), Cell::Value);
        assert_eq!(cell("lispico-cl", "(cond () 1)", 7), Cell::Value);
        assert_eq!(cell("lispico-cl", "#() 1", 2), Cell::Value);
    }

    #[test]
    fn definition_name_positions_use_declared_cell() {
        assert_eq!(cell("lispico-cl", "(defun  ())", 7), Cell::Function);
        assert_eq!(cell("lispico-cl", "(defn  [])", 6), Cell::Function);
        assert_eq!(cell("lispico-cl", "(defmacro  ())", 10), Cell::Function);
        assert_eq!(cell("lispico-cl", "(def  1)", 5), Cell::Value);
        assert_eq!(cell("lispico-cl", "(defun noise (x) 1)", 16), Cell::Value);
    }

    #[test]
    fn heads_and_plain_positions_keep_cells() {
        assert_eq!(cell("lispico-cl", "(noise 1)", 3), Cell::Function);
        assert_eq!(cell("lispico-cl", "(noise )", 7), Cell::Value);
        assert_eq!(cell("lispico-cl", "(noise 1)", 7), Cell::Value);
        assert_eq!(cell("common-lisp", "(noise 1)", 3), Cell::Value);
    }
}
