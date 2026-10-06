use lsp_types::{
    FoldingRange, FoldingRangeKind, FoldingRangeParams, Range, SelectionRange,
    SelectionRangeParams, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensLegend, SemanticTokensParams, SemanticTokensRangeParams,
    SemanticTokensRangeResult, SemanticTokensResult,
};
use rustc_hash::FxHashMap;

use super::FeatureResult;
use crate::analysis::{Target, symbol_like};
use crate::dialect::SymbolKind;
use crate::document::Document;
use crate::session::Session;
use crate::syntax::{NodeKind, TokenKind, Tree};

const TYPES: [SemanticTokenType; 12] = [
    SemanticTokenType::NAMESPACE,
    SemanticTokenType::TYPE,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::MACRO,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::PARAMETER,
    SemanticTokenType::PROPERTY,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::COMMENT,
    SemanticTokenType::STRING,
    SemanticTokenType::NUMBER,
    SemanticTokenType::REGEXP,
];

#[derive(Clone, Copy)]
#[repr(u32)]
enum Ty {
    Namespace,
    Type,
    Function,
    Macro,
    Variable,
    Parameter,
    Property,
    Keyword,
    Comment,
    Str,
    Number,
    Regexp,
}

const DECLARATION: u32 = 1;
const DEFINITION: u32 = 2;
const READONLY: u32 = 4;
const LIBRARY: u32 = 8;

pub(crate) fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TYPES.to_vec(),
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,
            SemanticTokenModifier::DEFINITION,
            SemanticTokenModifier::READONLY,
            SemanticTokenModifier::DEFAULT_LIBRARY,
        ],
    }
}

pub(crate) fn folding_ranges(
    s: &Session,
    p: FoldingRangeParams,
) -> FeatureResult<Option<Vec<FoldingRange>>> {
    let Some((_, doc)) = s.document(&p.text_document.uri) else {
        return Ok(None);
    };
    let tree = doc.tree();
    let line = |off: u32| doc.position(off, s.enc).line;
    let mut by_start: FxHashMap<u32, (u32, Option<FoldingRangeKind>)> = FxHashMap::default();
    let mut add = |start: u32, end: u32, kind: Option<FoldingRangeKind>| {
        if end > start {
            let e = by_start.entry(start).or_insert((end, kind.clone()));
            if end > e.0 {
                *e = (end, kind);
            }
        }
    };
    for id in tree.preorder(Tree::ROOT).skip(1) {
        let n = tree.node(id);
        if matches!(n.kind, NodeKind::List(_)) {
            add(
                line(n.start),
                line(n.end.saturating_sub(1).max(n.start)),
                None,
            );
        }
    }
    let mut run: Option<(u32, u32)> = None;
    for t in tree.tokens() {
        match t.kind {
            TokenKind::BlockComment => {
                add(
                    line(t.start),
                    line(t.end - 1),
                    Some(FoldingRangeKind::Comment),
                );
            }
            TokenKind::LineComment => {
                let l = line(t.start);
                run = match run {
                    Some((st, e)) if e + 1 == l => Some((st, l)),
                    Some((st, e)) => {
                        add(st, e, Some(FoldingRangeKind::Comment));
                        Some((l, l))
                    }
                    None => Some((l, l)),
                };
            }
            _ => {}
        }
    }
    if let Some((st, e)) = run {
        add(st, e, Some(FoldingRangeKind::Comment));
    }
    let mut out: Vec<FoldingRange> = by_start
        .into_iter()
        .map(|(start, (end, kind))| FoldingRange {
            start_line: start,
            end_line: end,
            kind,
            ..Default::default()
        })
        .collect();
    out.sort_by_key(|f| f.start_line);
    Ok(Some(out))
}

pub(crate) fn selection_ranges(
    s: &Session,
    p: SelectionRangeParams,
) -> FeatureResult<Option<Vec<SelectionRange>>> {
    let Some((_, doc)) = s.document(&p.text_document.uri) else {
        return Ok(None);
    };
    let tree = doc.tree();
    let out = p
        .positions
        .into_iter()
        .map(|pos| {
            let offset = doc.offset(pos, s.enc);
            let mut ranges: Vec<Range> = Vec::new();
            for id in tree.ancestors(tree.node_at(offset)) {
                if id == Tree::ROOT {
                    break;
                }
                let n = tree.node(id);
                let r = doc.range(n.start, n.end, s.enc);
                if ranges.last() != Some(&r) {
                    ranges.push(r);
                }
            }
            if ranges.is_empty() {
                ranges.push(Range::new(pos, pos));
            }
            ranges
                .into_iter()
                .rev()
                .fold(None, |parent, range| {
                    Some(SelectionRange {
                        range,
                        parent: parent.map(Box::new),
                    })
                })
                .expect("at least one range")
        })
        .collect();
    Ok(Some(out))
}

pub(crate) fn semantic_tokens_full(
    s: &Session,
    p: SemanticTokensParams,
) -> FeatureResult<Option<SemanticTokensResult>> {
    let Some((_, doc)) = s.document(&p.text_document.uri) else {
        return Ok(None);
    };
    let data = semantic_tokens(s, doc, None);
    Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    })))
}

pub(crate) fn semantic_tokens_range(
    s: &Session,
    p: SemanticTokensRangeParams,
) -> FeatureResult<Option<SemanticTokensRangeResult>> {
    let Some((_, doc)) = s.document(&p.text_document.uri) else {
        return Ok(None);
    };
    let range = (
        doc.offset(p.range.start, s.enc),
        doc.offset(p.range.end, s.enc),
    );
    let data = semantic_tokens(s, doc, Some(range));
    Ok(Some(SemanticTokensRangeResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    })))
}

fn semantic_tokens(s: &Session, doc: &Document, range: Option<(u32, u32)>) -> Vec<SemanticToken> {
    let tree = doc.tree();
    let a = doc.analysis();
    let d = &doc.dialect;
    let by_end: FxHashMap<u32, usize> = a
        .occurrences
        .iter()
        .enumerate()
        .map(|(i, o)| (o.end, i))
        .collect();
    let def_names: FxHashMap<u32, SymbolKind> =
        a.defs.iter().map(|x| (x.name_start, x.kind)).collect();
    let mut def_kinds: FxHashMap<&str, SymbolKind> = FxHashMap::default();
    for x in &a.defs {
        def_kinds.entry(x.key.as_str()).or_insert(x.kind);
    }
    let commented: Vec<(u32, u32)> = tree
        .preorder(Tree::ROOT)
        .filter(|&id| tree.node(id).kind == NodeKind::DatumComment)
        .map(|id| (tree.node(id).start, tree.node(id).end))
        .collect();

    let tokens = tree.tokens();
    let is_regex_prefix = |i: usize| {
        tokens.get(i).is_some_and(|p| {
            p.kind == TokenKind::Prefix && d.is_regexp_string_prefix(tree.token_text(p))
        }) && tokens
            .get(i + 1)
            .is_some_and(|n| n.kind == TokenKind::String)
    };
    let mut raw: Vec<(u32, u32, Ty, u32)> = Vec::new();
    let mut next_comment = 0;
    for (i, t) in tokens.iter().enumerate() {
        if let Some((st, e)) = range
            && (t.end <= st || t.start >= e)
        {
            continue;
        }
        while commented
            .get(next_comment)
            .is_some_and(|&(_, e)| e <= t.start)
        {
            next_comment += 1;
        }
        if let Some(&(cst, ce)) = commented
            .get(next_comment)
            .filter(|&&(cst, ce)| cst <= t.start && t.end <= ce)
        {
            if t.start == cst {
                raw.push((cst, ce, Ty::Comment, 0));
            }
            continue;
        }
        let text = tree.token_text(t);
        match t.kind {
            TokenKind::LineComment | TokenKind::BlockComment => {
                raw.push((t.start, t.end, Ty::Comment, 0));
            }
            TokenKind::String | TokenKind::Char => {
                let regex = i > 0 && is_regex_prefix(i - 1);
                raw.push((t.start, t.end, if regex { Ty::Regexp } else { Ty::Str }, 0));
            }
            TokenKind::Atom => {
                let key = d.normalize(text);
                if d.is_keyword(text) {
                    raw.push((t.start, t.end, Ty::Property, 0));
                } else if d.is_constant(&key) {
                    raw.push((t.start, t.end, Ty::Variable, READONLY | LIBRARY));
                } else if !symbol_like(d, text) {
                    if !text.starts_with('#') {
                        raw.push((t.start, t.end, Ty::Number, 0));
                    }
                } else if let Some(&i) = by_end.get(&t.end) {
                    let o = &a.occurrences[i];
                    if o.start > t.start {
                        let q = o.qualifier.as_deref().unwrap_or_default().len() as u32;
                        raw.push((t.start, t.start + q, Ty::Namespace, 0));
                    }
                    if let Some((ty, m)) = classify(s, doc, o, &def_names, &def_kinds) {
                        raw.push((o.start, o.end, ty, m));
                    }
                }
            }
            TokenKind::Prefix if is_regex_prefix(i) => {
                raw.push((t.start, t.end, Ty::Regexp, 0));
            }
            _ => {}
        }
    }
    encode(doc, s.enc, raw)
}

fn classify(
    s: &Session,
    doc: &Document,
    o: &crate::analysis::Occurrence,
    def_names: &FxHashMap<u32, SymbolKind>,
    def_kinds: &FxHashMap<&str, SymbolKind>,
) -> Option<(Ty, u32)> {
    let a = doc.analysis();
    let d = &doc.dialect;
    if let Target::Local(b) = o.target {
        let decl = a.binders[b as usize].start == o.start;
        return Some((Ty::Parameter, if decl { DECLARATION } else { 0 }));
    }
    if let Some(&kind) = def_names.get(&o.start) {
        let (ty, m) = kind_type(kind);
        return Some((ty, m | DEFINITION));
    }
    if o.qualifier.is_none() && d.is_special_form(&o.key) {
        return Some((Ty::Keyword, 0));
    }
    let kind = def_kinds.get(o.key.as_str()).copied().or_else(|| {
        s.index
            .defs_named(&o.key)
            .find(|(f, _)| f.dialect.name == d.name)
            .map(|(_, x)| x.kind)
    });
    if let Some(kind) = kind {
        return Some(kind_type(kind));
    }
    d.is_builtin(&o.key).then_some((Ty::Function, LIBRARY))
}

fn kind_type(kind: SymbolKind) -> (Ty, u32) {
    match kind {
        SymbolKind::Function | SymbolKind::Method | SymbolKind::Test => (Ty::Function, 0),
        SymbolKind::Macro => (Ty::Macro, 0),
        SymbolKind::Variable => (Ty::Variable, 0),
        SymbolKind::Constant => (Ty::Variable, READONLY),
        SymbolKind::Class | SymbolKind::Struct | SymbolKind::Type | SymbolKind::Interface => {
            (Ty::Type, 0)
        }
        SymbolKind::Module => (Ty::Namespace, 0),
    }
}

/// Splits multi-line tokens per line and delta-encodes them.
fn encode(
    doc: &Document,
    enc: crate::document::Encoding,
    raw: Vec<(u32, u32, Ty, u32)>,
) -> Vec<SemanticToken> {
    let mut out = Vec::with_capacity(raw.len());
    let (mut prev_line, mut prev_col) = (0u32, 0u32);
    let mut push = |start: u32, end: u32, ty: Ty, m: u32| {
        let st = doc.position(start, enc);
        let e = doc.position(end, enc);
        debug_assert_eq!(st.line, e.line);
        if e.character <= st.character {
            return;
        }
        let delta_line = st.line - prev_line;
        let delta_start = if delta_line == 0 {
            st.character - prev_col
        } else {
            st.character
        };
        out.push(SemanticToken {
            delta_line,
            delta_start,
            length: e.character - st.character,
            token_type: ty as u32,
            token_modifiers_bitset: m,
        });
        (prev_line, prev_col) = (st.line, st.character);
    };
    let text = doc.text();
    for (start, end, ty, m) in raw {
        let mut st = start;
        for (i, _) in text[start as usize..end as usize].match_indices('\n') {
            let nl = start + i as u32;
            let seg_end = if nl > st && text.as_bytes()[nl as usize - 1] == b'\r' {
                nl - 1
            } else {
                nl
            };
            push(st, seg_end, ty, m);
            st = nl + 1;
        }
        push(st, end, ty, m);
    }
    out
}
