use rustc_hash::FxHashSet;

use crate::analysis::{Analysis, Target};
use crate::config::{Diagnostics, Level};
use crate::dialect::{Cell, Dialect, SymbolKind};
use crate::syntax::{Delim, NodeKind, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Hint => "hint",
        }
    }

    fn from_level(level: Level) -> Option<Self> {
        match level {
            Level::Off => None,
            Level::Hint => Some(Self::Hint),
            Level::Info => Some(Self::Info),
            Level::Warning => Some(Self::Warning),
            Level::Error => Some(Self::Error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub start: u32,
    pub end: u32,
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub unnecessary: bool,
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
            unnecessary: false,
        })
        .collect()
}

/// All diagnostics for one file. `is_defined` answers whether a normalized name has a
/// definition in the workspace (same dialect).
pub fn check(
    tree: &Tree,
    analysis: &Analysis,
    dialect: &Dialect,
    cfg: &Diagnostics,
    is_defined: impl Fn(&str) -> bool,
) -> Vec<Diagnostic> {
    if !cfg.enable {
        return Vec::new();
    }
    let mut out = syntax(tree);
    if let Some(severity) = Severity::from_level(cfg.unused_binding) {
        unused_bindings(analysis, cfg, severity, &mut out);
    }
    if let Some(severity) = Severity::from_level(cfg.duplicate_definition) {
        duplicate_definitions(analysis, dialect, severity, &mut out);
    }
    if let Some(severity) = Severity::from_level(cfg.unresolved_call) {
        unresolved_calls(
            tree,
            analysis,
            dialect,
            cfg,
            severity,
            &is_defined,
            &mut out,
        );
    }
    out.sort_by_key(|d| (d.start, d.end));
    out
}

fn unused_bindings(a: &Analysis, cfg: &Diagnostics, severity: Severity, out: &mut Vec<Diagnostic>) {
    let mut used = vec![false; a.binders.len()];
    for o in &a.occurrences {
        if let Target::Local(b) = o.target
            && a.binders[b as usize].start != o.start
        {
            used[b as usize] = true;
        }
    }
    for (b, used) in a.binders.iter().zip(used) {
        if used || (!cfg.ignore_prefix.is_empty() && b.name.starts_with(&cfg.ignore_prefix)) {
            continue;
        }
        out.push(Diagnostic {
            start: b.start,
            end: b.end,
            severity,
            code: "unused-binding",
            message: format!("`{}` is never used", b.name),
            unnecessary: true,
        });
    }
}

fn duplicate_definitions(a: &Analysis, d: &Dialect, severity: Severity, out: &mut Vec<Diagnostic>) {
    let mut seen: FxHashSet<(&str, SymbolKind, Option<&str>, Option<Cell>)> = FxHashSet::default();
    for def in &a.defs {
        if def.kind == SymbolKind::Method {
            continue;
        }
        let cell = d.function_cells.then_some(def.cell);
        if !seen.insert((def.key.as_str(), def.kind, def.namespace.as_deref(), cell)) {
            out.push(Diagnostic {
                start: def.name_start,
                end: def.name_end,
                severity,
                code: "duplicate-definition",
                message: format!("`{}` is already defined in this file", def.name),
                unnecessary: false,
            });
        }
    }
}

fn unresolved_calls(
    tree: &Tree,
    a: &Analysis,
    d: &Dialect,
    cfg: &Diagnostics,
    severity: Severity,
    is_defined: &impl Fn(&str) -> bool,
    out: &mut Vec<Diagnostic>,
) {
    let known: FxHashSet<String> = cfg
        .known_symbols
        .iter()
        .map(|s| d.normalize(s).into_owned())
        .collect();
    let local: FxHashSet<&str> = a
        .defs
        .iter()
        .filter(|def| !d.function_cells || def.cell == Cell::Function)
        .map(|def| def.key.as_str())
        .collect();
    for o in &a.occurrences {
        if o.target != Target::Global || o.qualifier.is_some() {
            continue;
        }
        if d.function_cells && o.cell != Cell::Function {
            continue;
        }
        let Some(parent) = tree.parent(o.node) else {
            continue;
        };
        if tree.node(parent).kind != NodeKind::List(Delim::Paren)
            || tree.child(parent, 0) != Some(o.node)
            || in_data(tree, d, parent)
        {
            continue;
        }
        let key = o.key.as_str();
        if d.is_special_form(key)
            || d.is_builtin(key)
            || d.is_constant(key)
            || known.contains(key)
            || local.contains(key)
            || is_defined(key)
        {
            continue;
        }
        out.push(Diagnostic {
            start: o.start,
            end: o.end,
            severity,
            code: "unresolved-call",
            message: format!(
                "`{}` is not defined in the workspace",
                tree.node_text(o.node)
            ),
            unnecessary: false,
        });
    }
}

/// True when `list` sits inside quoted data or in the arguments of a data form.
fn in_data(tree: &Tree, d: &Dialect, list: u32) -> bool {
    let mut child = list;
    for anc in tree.ancestors(list).skip(1) {
        match tree.node(anc).kind {
            NodeKind::Prefix if matches!(tree.prefix_text(anc), "'" | "`") => return true,
            NodeKind::List(Delim::Paren) => {
                if tree.child(anc, 0) != Some(child)
                    && let Some(head) = tree.head(anc)
                    && d.is_data_form(&d.normalize(head))
                {
                    return true;
                }
            }
            _ => {}
        }
        child = anc;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::Dialects;

    fn run(dialect: &str, src: &str, cfg: Diagnostics) -> Vec<(&'static str, String)> {
        let ds = Dialects::builtin();
        let d = ds.get(dialect).unwrap();
        let t = Tree::parse(src.to_owned(), d);
        let a = Analysis::new(&t, d);
        check(&t, &a, d, &cfg, |k| k == "helper")
            .into_iter()
            .map(|x| (x.code, src[x.start as usize..x.end as usize].to_owned()))
            .collect()
    }

    fn lints() -> Diagnostics {
        Diagnostics {
            unresolved_call: Level::Warning,
            ..Diagnostics::default()
        }
    }

    #[test]
    fn unused_binding() {
        assert_eq!(
            run("common-lisp", "(let ((a 1) (b 2)) a)", lints()),
            [("unused-binding", "b".to_owned())]
        );
        assert!(run("clojure", "(fn [_x] 1)", lints()).is_empty());
        assert!(run("common-lisp", "(defun f (x) (declare (ignore x)))", lints()).is_empty());
    }

    #[test]
    fn duplicate_definition() {
        assert_eq!(
            run(
                "common-lisp",
                "(defun f ()) (defun f ()) (defmethod m ()) (defmethod m ())",
                lints()
            ),
            [("duplicate-definition", "f".to_owned())]
        );
        assert!(
            run(
                "common-lisp",
                "(in-package :a) (defun f ()) (in-package :b) (defun f ())",
                lints()
            )
            .is_empty()
        );
    }

    #[test]
    fn unresolved_call() {
        assert_eq!(
            run(
                "common-lisp",
                "(frobnicate (car nil) (helper) (local))\n(defun local ())",
                lints()
            ),
            [("unresolved-call", "frobnicate".to_owned())]
        );
        assert!(run("common-lisp", "'(frobnicate 1) `(a ,(b))", lints()).is_empty());
        assert!(
            run(
                "common-lisp",
                "(case x (red 1)) (flet ((g () 1)) (g))",
                lints()
            )
            .is_empty()
        );
        assert!(
            run(
                "clojure",
                "(ns a (:require [b.c :as c])) (c/thing) (#(inc %) 1)",
                lints()
            )
            .is_empty()
        );
    }

    #[test]
    fn known_symbols_and_off() {
        let cfg = Diagnostics {
            known_symbols: vec!["FROB".into()],
            ..lints()
        };
        assert!(run("common-lisp", "(frob)", cfg).is_empty());
        let cfg = Diagnostics {
            unused_binding: Level::Off,
            ..Diagnostics::default()
        };
        assert!(run("common-lisp", "(let ((a 1)) 2)", cfg).is_empty());
        let cfg = Diagnostics {
            enable: false,
            ..lints()
        };
        assert!(run("common-lisp", "(", cfg).is_empty());
    }

    #[test]
    fn lisp2_def_and_defun_not_duplicate() {
        assert!(run("lispico-cl", "(def n 1) (defun n () 2)", lints()).is_empty());
        let cfg = Diagnostics {
            unresolved_call: Level::Off,
            ..lints()
        };
        assert_eq!(
            run("common-lisp", "(defun f ()) (defthing f)", cfg),
            [("duplicate-definition", "f".to_owned())]
        );
    }

    #[test]
    fn lisp2_cond_and_vector_heads_not_calls() {
        assert!(
            run(
                "lispico-cl",
                "(def x 1) (cond (x 2)) (let ((y 1)) #(y 2))",
                lints()
            )
            .is_empty()
        );
        assert_eq!(
            run("lispico-cl", "(cond ((frob) 1))", lints()),
            [("unresolved-call", "frob".to_owned())]
        );
    }

    #[test]
    fn lisp2_value_binding_in_call_position() {
        let cfg = Diagnostics {
            unused_binding: Level::Off,
            ..lints()
        };
        assert_eq!(
            run("lispico-cl", "(let ((k 1)) (k))", cfg),
            [("unresolved-call", "k".to_owned())]
        );
    }

    #[test]
    fn syntax_errors_always_reported() {
        let cfg = Diagnostics {
            unused_binding: Level::Off,
            duplicate_definition: Level::Off,
            ..Diagnostics::default()
        };
        assert_eq!(
            run("common-lisp", "(defun f (x)", cfg),
            [("unclosed-delimiter", "(".to_owned())]
        );
    }
}
