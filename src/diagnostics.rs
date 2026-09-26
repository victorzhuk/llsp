use crate::syntax::Tree;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub start: u32,
    pub end: u32,
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

pub fn syntax(tree: &Tree) -> Vec<Diagnostic> {
    tree.errors()
        .iter()
        .map(|e| Diagnostic {
            start: e.start,
            end: e.end,
            severity: Severity::Error,
            code: e.kind.code(),
            message: e.kind.message().to_owned(),
        })
        .collect()
}
