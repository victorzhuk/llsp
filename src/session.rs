//! Editor-session state shared by every feature: the open documents, the
//! workspace index, the effective settings and the negotiated encoding.
//! Transport, scheduling and lifecycle live in [`crate::server::Server`].

use lsp_types::{Location, Position, Range, Uri};

use rustc_hash::FxHashMap;

use crate::analysis::{Occurrence, Target};
use crate::dialect::Cell;
use crate::document::{Document, Encoding, lines_position, normalize_uri};
use crate::workspace::{FileSummary, Index};

pub(crate) struct Session {
    pub(crate) settings: crate::config::Settings,
    pub(crate) enc: Encoding,
    pub(crate) roots: Vec<std::path::PathBuf>,
    pub(crate) docs: FxHashMap<Uri, Document>,
    pub(crate) index: Index,
}

impl Session {
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
                explicit: a.explicit_namespace(&doc.dialect, occ),
                context: a.namespace_at(occ.start).map(str::to_owned),
                cell: occ.cell,
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

    /// Distinct namespaces with a definition of `key` in files of `doc`'s dialect.
    pub(crate) fn defined_namespaces<'a>(
        &'a self,
        doc: &'a Document,
        key: &'a str,
        cell: Cell,
    ) -> Vec<Option<&'a str>> {
        let mut v: Vec<Option<&str>> = self
            .index
            .defs_in(key, &doc.dialect, cell)
            .map(|(_, d)| d.namespace.as_deref())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }
}

/// Indexed files that share the dialect of `doc`.
pub(crate) fn same_dialect<'a>(doc: &'a Document) -> impl Fn(&FileSummary) -> bool + 'a {
    move |f| f.dialect.name == doc.dialect.name
}

/// Namespace a global reference resolves to, given the namespaces that define its name:
/// the explicit one, else the current one if it defines the name, else the only one.
/// `None` when that is ambiguous.
pub(crate) fn owner<'a>(
    defined: &[Option<&'a str>],
    explicit: Option<&'a str>,
    context: Option<&'a str>,
) -> Option<Option<&'a str>> {
    if explicit.is_some() {
        return Some(explicit);
    }
    if defined.contains(&context) {
        return Some(context);
    }
    match defined {
        [only] => Some(*only),
        _ => None,
    }
}

/// What the symbol under the cursor refers to.
pub(crate) enum Sym {
    Local(u32),
    Global {
        key: String,
        /// Namespace named by a qualifier or `:refer`.
        explicit: Option<String>,
        /// Namespace current at the occurrence.
        context: Option<String>,
        /// Value or function position of the occurrence.
        cell: Cell,
    },
}
