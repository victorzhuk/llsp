pub(crate) mod assist;
pub(crate) mod formatting;
pub(crate) mod navigation;
pub(crate) mod structure;
pub(crate) mod symbols;

pub(crate) use structure::legend;

use lsp_types::SymbolKind as K;

use crate::dialect::SymbolKind;

/// Feature-level failure, mapped to LSP error codes at the dispatch site in
/// [`crate::server::Server`].
#[derive(Debug)]
pub(crate) enum FeatureError {
    /// The client sent parameters the feature cannot interpret (-32602).
    Invalid(String),
    /// The request is refused by policy, e.g. an ambiguous rename (-32003).
    Failed(String),
}

pub(crate) type FeatureResult<T> = Result<T, FeatureError>;

pub(crate) fn lsp_symbol_kind(kind: SymbolKind) -> lsp_types::SymbolKind {
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
