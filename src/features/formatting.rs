use lsp_types::{DocumentFormattingParams, DocumentRangeFormattingParams, TextEdit};

use crate::document::Document;
use crate::format;
use crate::server::{HandlerResult, Server};

impl Server {
    pub(crate) fn formatting(
        &mut self,
        p: DocumentFormattingParams,
    ) -> HandlerResult<Option<Vec<TextEdit>>> {
        let Some((_, doc)) = self.document(&p.text_document.uri) else {
            return Ok(None);
        };
        Ok(Some(self.format_edits(doc, None)))
    }

    pub(crate) fn range_formatting(
        &mut self,
        p: DocumentRangeFormattingParams,
    ) -> HandlerResult<Option<Vec<TextEdit>>> {
        let Some((_, doc)) = self.document(&p.text_document.uri) else {
            return Ok(None);
        };
        Ok(Some(self.format_edits(
            doc,
            Some((p.range.start.line, p.range.end.line)),
        )))
    }

    fn format_edits(&self, doc: &Document, lines: Option<(u32, u32)>) -> Vec<TextEdit> {
        let name = &doc.dialect.name;
        let hints = |key: &str| {
            self.index
                .defs_named(key)
                .filter(|(f, _)| &f.dialect.name == name)
                .find_map(|(_, d)| d.indent)
        };
        format::format(
            doc.tree(),
            &doc.dialect,
            &self.settings.config.format,
            &hints,
            lines,
        )
        .into_iter()
        .map(|e| TextEdit::new(doc.range(e.start, e.end, self.enc), e.text))
        .collect()
    }
}
