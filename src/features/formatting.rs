use lsp_types::{DocumentFormattingParams, DocumentRangeFormattingParams, TextEdit};

use super::FeatureResult;
use crate::document::Document;
use crate::format;
use crate::session::Session;

pub(crate) fn formatting(
    s: &Session,
    p: DocumentFormattingParams,
) -> FeatureResult<Option<Vec<TextEdit>>> {
    let Some((_, doc)) = s.document(&p.text_document.uri) else {
        return Ok(None);
    };
    Ok(Some(format_edits(s, doc, None)))
}

pub(crate) fn range_formatting(
    s: &Session,
    p: DocumentRangeFormattingParams,
) -> FeatureResult<Option<Vec<TextEdit>>> {
    let Some((_, doc)) = s.document(&p.text_document.uri) else {
        return Ok(None);
    };
    Ok(Some(format_edits(
        s,
        doc,
        Some((p.range.start.line, p.range.end.line)),
    )))
}

fn format_edits(s: &Session, doc: &Document, lines: Option<(u32, u32)>) -> Vec<TextEdit> {
    let hints = crate::workspace::indent_hints(&s.index, &doc.dialect);
    format::format(
        doc.tree(),
        &doc.dialect,
        &s.settings.config.format,
        &hints,
        lines,
    )
    .into_iter()
    .map(|e| TextEdit::new(doc.range(e.start, e.end, s.enc), e.text))
    .collect()
}
