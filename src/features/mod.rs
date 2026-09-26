mod assist;
mod formatting;
mod navigation;
mod structure;
mod symbols;

pub(crate) use structure::legend;
pub(crate) use symbols::score_lowercase;

use lsp_types::{Location, Position, Range, Uri};

use crate::analysis::{Occurrence, Target};
use crate::dialect::SymbolKind;
use crate::document::{Document, lines_position, normalize_uri};
use crate::server::Server;
use crate::workspace::FileSummary;

/// What the symbol under the cursor refers to.
pub(crate) enum Sym {
    Local(u32),
    Global {
        key: String,
        qualifier: Option<String>,
    },
}

impl Server {
    pub(crate) fn document(&self, uri: &Uri) -> Option<(Uri, &Document)> {
        let key = normalize_uri(uri);
        let doc = self.docs.get(&key).filter(|d| !d.oversized())?;
        Some((key, doc))
    }

    pub(crate) fn symbol_at<'a>(
        &self,
        doc: &'a Document,
        pos: Position,
    ) -> Option<(Sym, &'a Occurrence)> {
        let offset = doc.offset(pos, self.enc);
        let a = doc.analysis();
        let occ = a.occurrence_at(offset)?;
        let sym = match occ.target {
            Target::Local(b) => Sym::Local(b),
            Target::Global => Sym::Global {
                key: occ.key.clone(),
                qualifier: occ
                    .qualifier
                    .as_deref()
                    .map(|q| doc.dialect.normalize(a.resolve_qualifier(q)).into_owned()),
            },
        };
        Some((sym, occ))
    }

    /// Location in an indexed file, using live positions for open documents.
    pub(crate) fn location(&self, file: &FileSummary, start: u32, end: u32) -> Location {
        self.locator(file)(start, end)
    }

    /// Builds locations in one file, looking up its open document once.
    pub(crate) fn locator<'a>(
        &'a self,
        file: &'a FileSummary,
    ) -> impl Fn(u32, u32) -> Location + 'a {
        let doc = self.docs.get(&file.uri);
        let enc = self.enc;
        move |start, end| match doc {
            Some(doc) => Location::new(doc.client_uri.clone(), doc.range(start, end, enc)),
            None => Location::new(
                file.uri.clone(),
                Range::new(
                    lines_position(&file.lines, start, enc),
                    lines_position(&file.lines, end, enc),
                ),
            ),
        }
    }

    /// Indexed files that share the dialect of `doc`.
    pub(crate) fn same_dialect<'a>(
        &'a self,
        doc: &'a Document,
    ) -> impl Fn(&FileSummary) -> bool + 'a {
        move |f| f.dialect.name == doc.dialect.name
    }
}

pub(crate) fn lsp_symbol_kind(kind: SymbolKind) -> lsp_types::SymbolKind {
    use lsp_types::SymbolKind as K;
    match kind {
        SymbolKind::Function => K::FUNCTION,
        SymbolKind::Macro => K::OPERATOR,
        SymbolKind::Variable => K::VARIABLE,
        SymbolKind::Constant => K::CONSTANT,
        SymbolKind::Class => K::CLASS,
        SymbolKind::Struct => K::STRUCT,
        SymbolKind::Type => K::TYPE_PARAMETER,
        SymbolKind::Interface => K::INTERFACE,
        SymbolKind::Method => K::METHOD,
        SymbolKind::Module => K::MODULE,
        SymbolKind::Test => K::EVENT,
    }
}
