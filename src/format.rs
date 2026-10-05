use crate::analysis::symbol_like;
use crate::config::Format;
use crate::dialect::Dialect;
use crate::syntax::{Delim, NodeId, NodeKind, TokenKind, Tree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: u32,
    pub end: u32,
    pub text: String,
}

/// Re-indents `tree`. `hints` maps a normalized head name to an indent spec declared in code.
/// `only_lines` limits the returned edits to an inclusive line range.
pub fn format(
    tree: &Tree,
    dialect: &Dialect,
    cfg: &Format,
    hints: &dyn Fn(&str) -> Option<u32>,
    only_lines: Option<(u32, u32)>,
) -> Vec<Edit> {
    let text = tree.text();
    let starts: Vec<u32> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i as u32 + 1))
        .collect();
    let f = Formatter {
        tree,
        dialect,
        cfg,
        hints,
        text,
        starts: &starts,
    };
    f.run(only_lines)
}

pub fn apply(text: &str, edits: &[Edit]) -> String {
    let mut out = text.to_owned();
    for e in edits.iter().rev() {
        out.replace_range(e.start as usize..e.end as usize, &e.text);
    }
    out
}

struct Formatter<'a> {
    tree: &'a Tree,
    dialect: &'a Dialect,
    cfg: &'a Format,
    hints: &'a dyn Fn(&str) -> Option<u32>,
    text: &'a str,
    starts: &'a [u32],
}

impl Formatter<'_> {
    fn run(&self, only_lines: Option<(u32, u32)>) -> Vec<Edit> {
        let mut shift = vec![0i64; self.starts.len()];
        let mut edits = Vec::new();
        for (l, &ls) in self.starts.iter().enumerate() {
            let le = self.line_end(l);
            let in_range = only_lines.is_none_or(|(a, b)| (a..=b).contains(&(l as u32)));
            let line = &self.text[ls as usize..le as usize];
            let ws = line.len() - line.trim_start_matches([' ', '\t']).len();
            let fs = ls + ws as u32;

            let mut leading = None;
            if !self.inside_literal(ls) {
                let target = if fs == le {
                    0
                } else {
                    (self.indent_for(fs, &shift) as u32).min(crate::config::MAX_INDENT) as usize
                };
                shift[l] = target as i64 - ws as i64;
                if line[..ws] != " ".repeat(target) {
                    leading = Some(Edit {
                        start: ls,
                        end: fs,
                        text: " ".repeat(target),
                    });
                }
            }
            let trailing =
                if self.cfg.trim_trailing_whitespace && fs < le && !self.inside_literal(le) {
                    let kept = line.trim_end_matches([' ', '\t']).len() as u32;
                    (ls + kept < le).then(|| Edit {
                        start: ls + kept,
                        end: le,
                        text: String::new(),
                    })
                } else {
                    None
                };
            if in_range {
                edits.extend(leading);
                edits.extend(trailing);
            }
        }
        edits
    }

    fn line_end(&self, l: usize) -> u32 {
        let end = self
            .starts
            .get(l + 1)
            .map_or(self.text.len() as u32, |&next| next - 1);
        if end > self.starts[l] && self.text.as_bytes()[end as usize - 1] == b'\r' {
            end - 1
        } else {
            end
        }
    }

    fn line_of(&self, off: u32) -> usize {
        self.starts.partition_point(|&s| s <= off) - 1
    }

    /// Column after formatting of an offset on an already processed line.
    fn col(&self, off: u32, shift: &[i64]) -> usize {
        let l = self.line_of(off);
        let chars = self.text[self.starts[l] as usize..off as usize]
            .chars()
            .count() as i64;
        (chars + shift[l]).max(0) as usize
    }

    fn inside_literal(&self, off: u32) -> bool {
        let tokens = self.tree.tokens();
        let i = tokens.partition_point(|t| t.end <= off);
        tokens.get(i).is_some_and(|t| {
            t.start < off && matches!(t.kind, TokenKind::String | TokenKind::BlockComment)
        })
    }

    fn indent_for(&self, fs: u32, shift: &[i64]) -> usize {
        let t = self.tree;
        let Some(list) = t.ancestors(t.node_at(fs)).find(|&id| {
            let n = t.node(id);
            matches!(n.kind, NodeKind::List(_)) && n.start < fs && (!n.closed || fs < n.end)
        }) else {
            return 0;
        };
        let open = self.col(t.node(list).start, shift);
        let kids = t.children(list);
        let idx = kids.iter().filter(|&&k| t.node(k).start < fs).count();
        if !matches!(t.node(list).kind, NodeKind::List(Delim::Paren)) || idx == 0 {
            return open + 1;
        }
        let head = kids[0];
        if let Some(n) = self.spec(head) {
            let arg = idx - 1;
            let extra = if arg < n as usize {
                self.cfg.distinguished_indent
            } else {
                self.cfg.body_indent
            };
            return open + extra as usize;
        }
        self.first_arg_on_head_line(kids, fs)
            .map_or(open + 1, |a| self.col(t.node(a).start, shift))
    }

    fn spec(&self, head: NodeId) -> Option<u32> {
        let text = self.tree.atom(head)?;
        if !symbol_like(self.dialect, text) {
            return None;
        }
        let key = self.dialect.normalize(text);
        (self.hints)(&key).or_else(|| self.dialect.indent_spec(&key))
    }

    fn first_arg_on_head_line(&self, kids: &[NodeId], fs: u32) -> Option<NodeId> {
        let t = self.tree;
        let (&head, &arg) = (kids.first()?, kids.get(1)?);
        let a = t.node(arg);
        (a.start < fs && self.line_of(a.start) == self.line_of(t.node(head).start)).then_some(arg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialect::Dialects;

    fn fmt_with(dialect: &str, src: &str, hints: &dyn Fn(&str) -> Option<u32>) -> String {
        let ds = Dialects::builtin();
        let d = ds.get(dialect).unwrap();
        let t = Tree::parse(src.to_owned(), d);
        let edits = format(&t, d, &Format::default(), hints, None);
        apply(src, &edits)
    }

    fn fmt(dialect: &str, src: &str) -> String {
        fmt_with(dialect, src, &|_| None)
    }

    #[test]
    fn body_indentation() {
        assert_eq!(
            fmt("common-lisp", "(defun f (x)\n(let ((y x))\ny))"),
            "(defun f (x)\n  (let ((y x))\n    y))"
        );
    }

    #[test]
    fn call_alignment() {
        assert_eq!(fmt("common-lisp", "(foo a\nb)"), "(foo a\n     b)");
        assert_eq!(fmt("common-lisp", "(foo\nb)"), "(foo\n b)");
        assert_eq!(
            fmt("common-lisp", "(if (a)\n(b)\n(c))"),
            "(if (a)\n    (b)\n    (c))"
        );
    }

    #[test]
    fn distinguished_arguments() {
        assert_eq!(
            fmt("common-lisp", "(with-slots (a b)\nobj\nbody)"),
            "(with-slots (a b)\n    obj\n  body)"
        );
        assert_eq!(
            fmt("emacs-lisp", "(if (a)\n(b)\n(c))"),
            "(if (a)\n    (b)\n  (c))"
        );
    }

    #[test]
    fn declared_indent_hint() {
        let hints = |k: &str| (k == "with-x").then_some(1);
        assert_eq!(
            fmt_with("emacs-lisp", "(with-x a\nbody)", &hints),
            "(with-x a\n  body)"
        );
        assert_eq!(
            fmt("emacs-lisp", "(with-x a\nbody)"),
            "(with-x a\n  body)",
            "with- prefix rule"
        );
        assert_eq!(fmt("emacs-lisp", "(frob a\nbody)"), "(frob a\n      body)");
    }

    #[test]
    fn data_brackets_and_clojure() {
        assert_eq!(
            fmt("clojure", "(defn f\n[x]\n(let [a 1\nb 2]\n{:a a\n:b b}))"),
            "(defn f\n  [x]\n  (let [a 1\n        b 2]\n    {:a a\n     :b b}))"
        );
        assert_eq!(
            fmt("clojure", "(ns a\n(:require [x]\n[y]))"),
            "(ns a\n  (:require [x]\n            [y]))"
        );
        assert_eq!(fmt("clojure", "(if x\ny\nz)"), "(if x\n  y\n  z)");
    }

    #[test]
    fn nested_shifts_are_consistent() {
        let src = "      (foo (bar a\n  b)\n c)";
        let once = fmt("common-lisp", src);
        assert_eq!(once, "(foo (bar a\n          b)\n     c)");
        assert_eq!(fmt("common-lisp", &once), once);
    }

    #[test]
    fn strings_and_comments_untouched() {
        assert_eq!(fmt("common-lisp", "(f \"a\n   b\")"), "(f \"a\n   b\")");
        assert_eq!(
            fmt("common-lisp", "#| x\n   y |#\n  (a)"),
            "#| x\n   y |#\n(a)"
        );
        assert_eq!(
            fmt("common-lisp", "(a   \n  ;; c  \nb)  "),
            "(a\n ;; c\n b)"
        );
    }

    #[test]
    fn closers_and_unclosed() {
        assert_eq!(
            fmt("common-lisp", "(let ((a 1))\na\n)"),
            "(let ((a 1))\n  a\n  )"
        );
        assert_eq!(fmt("common-lisp", "(defun f ()\n(g"), "(defun f ()\n  (g");
    }

    #[test]
    fn range_limits_edits() {
        let ds = Dialects::builtin();
        let d = ds.get("common-lisp").unwrap();
        let src = "(defun f ()\na\nb)";
        let t = Tree::parse(src.to_owned(), d);
        let edits = format(&t, d, &Format::default(), &|_| None, Some((2, 2)));
        assert_eq!(
            edits,
            [Edit {
                start: 14,
                end: 14,
                text: "  ".into()
            }]
        );
    }

    #[test]
    fn deep_nesting_stays_bounded() {
        let ds = Dialects::builtin();
        let d = ds.get("common-lisp").unwrap();
        let src = "(\n".repeat(10_000);
        let t = Tree::parse(src.clone(), d);
        let edits = format(&t, d, &Format::default(), &|_| None, None);
        let total: usize = edits.iter().map(|e| e.text.len()).sum();
        assert!(
            total <= 10_000 * crate::config::MAX_INDENT as usize,
            "edit text {total} is unbounded"
        );
    }

    mod props {
        use super::*;
        use proptest::prelude::*;

        fn source() -> impl Strategy<Value = String> {
            proptest::collection::vec(
                prop_oneof![
                    Just("(".to_owned()),
                    Just(")".to_owned()),
                    Just("[".to_owned()),
                    Just("]".to_owned()),
                    Just("\n".to_owned()),
                    Just("  ".to_owned()),
                    Just("\t".to_owned()),
                    Just(" ".to_owned()),
                    Just("\"s\ntr\"".to_owned()),
                    Just("; c\n".to_owned()),
                    Just("let".to_owned()),
                    Just("defun".to_owned()),
                    "[a-z]{1,4}",
                ],
                0..48,
            )
            .prop_map(|v| v.concat())
        }

        fn squash(s: &str) -> String {
            s.chars().filter(|c| !c.is_whitespace()).collect()
        }

        proptest! {
            #[test]
            fn idempotent_and_whitespace_only(src in source(), d in 0usize..3) {
                let dialect = ["common-lisp", "clojure", "scheme"][d];
                let once = fmt(dialect, &src);
                prop_assert_eq!(squash(&once), squash(&src));
                let twice = fmt(dialect, &once);
                prop_assert_eq!(twice, once);
            }
        }
    }
}
