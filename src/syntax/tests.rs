use super::*;
use crate::dialect::Dialects;

fn parse(dialect: &str, src: &str) -> Tree {
    let d = Dialects::builtin();
    Tree::parse(src.to_owned(), d.get(dialect).unwrap())
}

fn dump(tree: &Tree) -> String {
    fn go(t: &Tree, id: NodeId, out: &mut String) {
        let n = t.node(id);
        match n.kind {
            NodeKind::Atom | NodeKind::Str | NodeKind::Char => {
                out.push_str(&format!("{:?}{:?}", n.kind, t.node_text(id)));
                return;
            }
            NodeKind::Prefix => out.push_str(&format!("Prefix{:?}", t.prefix_text(id))),
            k => out.push_str(&format!("{k:?}")),
        }
        if !n.closed {
            out.push('!');
        }
        out.push('[');
        for (i, &c) in t.children(id).iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            go(t, c, out);
        }
        out.push(']');
    }
    let mut out = String::new();
    go(tree, Tree::ROOT, &mut out);
    for e in tree.errors() {
        out.push_str(&format!("\n{}@{}..{}", e.kind.code(), e.start, e.end));
    }
    out
}

fn tokens(dialect: &str, src: &str) -> Vec<(TokenKind, String)> {
    let t = parse(dialect, src);
    t.tokens()
        .iter()
        .filter(|k| k.kind != TokenKind::Whitespace)
        .map(|k| (k.kind, t.token_text(k).to_owned()))
        .collect()
}

#[test]
fn common_lisp_forms() {
    insta::assert_snapshot!(dump(&parse(
        "common-lisp",
        "(defun f (x) \"doc\" #'car #\\( #+sbcl (a) |a b| #|x #|y|# z|# #p\"/tmp\" #1=(a . #1#))"
    )));
}

#[test]
fn clojure_forms() {
    insta::assert_snapshot!(dump(&parse(
        "clojure",
        "(ns a.b) #_ (foo) {:a 1, :b [\\a \\newline]} ^:private #{x} @r #\"re\" #?(:clj 1) ~@xs"
    )));
}

#[test]
fn scheme_and_racket_forms() {
    insta::assert_snapshot!(dump(&parse(
        "racket",
        "#lang racket\n(define (f x) #;(ignored) [let ([y #t]) #\\space]) #hash((a . 1)) #:kw"
    )));
}

#[test]
fn emacs_lisp_chars() {
    assert_eq!(
        tokens("emacs-lisp", "?\\( ?a ?\\C-x [1 2]"),
        vec![
            (TokenKind::Char, "?\\(".into()),
            (TokenKind::Char, "?a".into()),
            (TokenKind::Char, "?\\C-x".into()),
            (TokenKind::Open, "[".into()),
            (TokenKind::Atom, "1".into()),
            (TokenKind::Atom, "2".into()),
            (TokenKind::Close, "]".into()),
        ]
    );
}

#[test]
fn fennel_hashfn() {
    insta::assert_snapshot!(dump(&parse("fennel", "(local f #(+ $1 1)) {:a [1]}")));
}

#[test]
fn janet_syntax() {
    assert_eq!(
        tokens("janet", "# note\n@[1] ``long `x` ``"),
        vec![
            (TokenKind::LineComment, "# note".into()),
            (TokenKind::Prefix, "@".into()),
            (TokenKind::Open, "[".into()),
            (TokenKind::Atom, "1".into()),
            (TokenKind::Close, "]".into()),
            (TokenKind::String, "``long `x` ``".into()),
        ]
    );
}

#[test]
fn clojure_commas_are_whitespace() {
    let t = parse("clojure", "{:a 1, :b 2}");
    let map = t.child(Tree::ROOT, 0).unwrap();
    assert_eq!(t.children(map).len(), 4);
}

#[test]
fn read_eval_is_syntax() {
    insta::assert_snapshot!(dump(&parse("common-lisp", "#.(delete-file \"x\")")));
}

#[test]
fn unclosed_list() {
    insta::assert_snapshot!(dump(&parse("common-lisp", "(defun f (x)")));
}

#[test]
fn mismatched_closer() {
    insta::assert_snapshot!(dump(&parse("clojure", "(foo]")));
}

#[test]
fn inner_unclosed_recovers_at_outer_closer() {
    insta::assert_snapshot!(dump(&parse("clojure", "[(a b] c")));
}

#[test]
fn unexpected_closer() {
    insta::assert_snapshot!(dump(&parse("scheme", "a) b")));
}

#[test]
fn unterminated_literals() {
    insta::assert_snapshot!(dump(&parse("common-lisp", "(a \"b")));
    insta::assert_snapshot!(dump(&parse("common-lisp", "#| a")));
}

#[test]
fn prefix_without_form() {
    insta::assert_snapshot!(dump(&parse("common-lisp", "(a ')")));
    insta::assert_snapshot!(dump(&parse("common-lisp", "'(a")));
}

#[test]
fn deep_nesting() {
    let src = "(".repeat(100_000);
    let t = parse("common-lisp", &src);
    assert_eq!(t.errors().len(), MAX_ERRORS);
    let mut depth = 0;
    let mut id = Tree::ROOT;
    while let Some(c) = t.child(id, 0) {
        id = c;
        depth += 1;
    }
    assert_eq!(depth, 100_000);
}

#[test]
fn node_at_finds_atoms_and_lists() {
    let t = parse("common-lisp", "(foo (bar baz))");
    assert_eq!(t.node_text(t.node_at(3)), "foo");
    assert_eq!(t.node_text(t.node_at(4)), "foo");
    assert_eq!(t.node_text(t.node_at(10)), "baz");
    assert_eq!(t.node_text(t.node_at(5)), "(bar baz)");
    assert_eq!(t.node_at(15), Tree::ROOT);
    assert_eq!(t.head(t.node_at(0)), Some("foo"));
}

#[test]
fn shebang_is_comment() {
    assert_eq!(
        tokens("janet", "#!/usr/bin/env janet\n(x)")[0],
        (TokenKind::LineComment, "#!/usr/bin/env janet".into())
    );
}

mod props {
    use super::*;
    use proptest::prelude::*;

    const DIALECTS: [&str; 7] = [
        "common-lisp",
        "clojure",
        "scheme",
        "racket",
        "emacs-lisp",
        "fennel",
        "janet",
    ];

    fn lisp_ish() -> impl Strategy<Value = String> {
        proptest::collection::vec(
            prop_oneof![
                Just("(".to_owned()),
                Just(")".to_owned()),
                Just("[".to_owned()),
                Just("]".to_owned()),
                Just("{".to_owned()),
                Just("}".to_owned()),
                Just("\"".to_owned()),
                Just("\\".to_owned()),
                Just("#".to_owned()),
                Just("|".to_owned()),
                Just(";".to_owned()),
                Just("'".to_owned()),
                Just("`".to_owned()),
                Just(",@".to_owned()),
                Just("?".to_owned()),
                Just("\n".to_owned()),
                Just(" ".to_owned()),
                "[a-z0-9:+#*-]{1,6}",
                "\\PC",
            ],
            0..64,
        )
        .prop_map(|v| v.concat())
    }

    proptest! {
        #[test]
        fn lossless_and_total(src in lisp_ish(), d in 0usize..7) {
            let t = parse(DIALECTS[d], &src);
            let joined: String = t.tokens().iter().map(|k| t.token_text(k)).collect();
            prop_assert_eq!(&joined, &src);
            for i in 0..t.node_count() as NodeId {
                let n = t.node(i);
                prop_assert!(n.start <= n.end && n.end as usize <= src.len());
                for &c in t.children(i) {
                    prop_assert_eq!(t.node(c).parent, i);
                }
            }
            prop_assert!(t.errors().len() <= MAX_ERRORS);
        }

        #[test]
        fn arbitrary_text_never_panics(src in "\\PC*", d in 0usize..7) {
            let t = parse(DIALECTS[d], &src);
            let joined: String = t.tokens().iter().map(|k| t.token_text(k)).collect();
            prop_assert_eq!(joined, src);
        }
    }
}
