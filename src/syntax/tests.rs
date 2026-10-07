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

#[test]
fn shebang_yields_to_invalid_opener() {
    let d = load_dialect(
        r##"[lx]
extends = "clojure"
[lx.reader]
invalid = ["#"]"##,
    );
    let t = parse_with(&d, "#!/bin/tool\n(ok)");
    let first = &t.tokens()[0];
    assert_eq!(first.kind, TokenKind::Atom);
    assert_eq!(t.token_text(first), "#");
    assert_eq!(
        t.errors(),
        [SyntaxError {
            kind: ErrorKind::InvalidSyntax,
            start: 0,
            end: 1
        }]
    );
}

fn load_dialect(src: &str) -> crate::dialect::Dialect {
    let overrides: toml::Table = toml::from_str(src).unwrap();
    let d = Dialects::load(&overrides).unwrap();
    let d: &crate::dialect::Dialect = d.get("lx").unwrap();
    d.clone()
}

fn parse_with(dialect: &crate::dialect::Dialect, src: &str) -> Tree {
    Tree::parse(src.to_owned(), dialect)
}

#[test]
fn invalid_openers_are_one_byte_atoms() {
    let d = load_dialect(
        r#"[lx]
extends = "common-lisp"
[lx.reader]
invalid = ["[", "]", "{", "}"]
terminators = "'`,[]{}""#,
    );
    let t = parse_with(&d, "(f [x])");
    let errs: Vec<_> = t
        .errors()
        .iter()
        .map(|e| (e.kind, e.start, e.end))
        .collect();
    assert_eq!(
        errs,
        [
            (ErrorKind::InvalidSyntax, 3, 4),
            (ErrorKind::InvalidSyntax, 5, 6)
        ]
    );
    let toks: Vec<_> = t
        .tokens()
        .iter()
        .filter(|k| k.kind != TokenKind::Whitespace)
        .map(|k| (k.kind, t.token_text(k).to_owned()))
        .collect();
    assert_eq!(
        toks,
        [
            (TokenKind::Open, "(".to_owned()),
            (TokenKind::Atom, "f".to_owned()),
            (TokenKind::Atom, "[".to_owned()),
            (TokenKind::Atom, "x".to_owned()),
            (TokenKind::Atom, "]".to_owned()),
            (TokenKind::Close, ")".to_owned()),
        ]
    );
    let kids: Vec<_> = (0..)
        .map_while(|i| t.child(Tree::ROOT, i))
        .map(|id| (t.node(id).kind, t.node(id).closed))
        .collect();
    assert_eq!(kids, [(NodeKind::List(Delim::Paren), true)]);
}

#[test]
fn invalid_opener_precedes_prefix_and_dispatch() {
    let d = load_dialect(
        r##"[lx]
extends = "clojure"
[lx.reader]
invalid = ["#"]"##,
    );
    let t = parse_with(&d, "#'f");
    let toks: Vec<_> = t
        .tokens()
        .iter()
        .filter(|k| k.kind != TokenKind::Whitespace)
        .map(|k| (k.kind, t.token_text(k).to_owned()))
        .collect();
    assert_eq!(
        toks,
        [
            (TokenKind::Atom, "#".to_owned()),
            (TokenKind::Prefix, "'".to_owned()),
            (TokenKind::Atom, "f".to_owned()),
        ]
    );
    assert_eq!(
        t.errors(),
        [SyntaxError {
            kind: ErrorKind::InvalidSyntax,
            start: 0,
            end: 1
        }]
    );

    let t = parse_with(&d, "#(1 2)");
    let toks: Vec<_> = t
        .tokens()
        .iter()
        .filter(|k| k.kind != TokenKind::Whitespace)
        .map(|k| (k.kind, t.token_text(k).to_owned()))
        .collect();
    assert_eq!(
        toks,
        [
            (TokenKind::Atom, "#".to_owned()),
            (TokenKind::Open, "(".to_owned()),
            (TokenKind::Atom, "1".to_owned()),
            (TokenKind::Atom, "2".to_owned()),
            (TokenKind::Close, ")".to_owned()),
        ]
    );
    for src in ["#_x", "#{1}"] {
        let t = parse_with(&d, src);
        let first = &t.tokens()[0];
        assert_eq!(first.kind, TokenKind::Atom);
        assert_eq!(t.token_text(first), "#");
    }
}

#[test]
fn empty_invalid_list_keeps_classification() {
    let cl = parse("common-lisp", "[x]");
    assert_eq!(dump(&cl), "Root[Atom\"[x]\"]");
    assert!(cl.errors().is_empty());
    let clj = parse("clojure", "{x}");
    assert_eq!(dump(&clj), "Root[List(Brace)[Atom\"x\"]]");
    assert!(clj.errors().is_empty());
}

mod props {
    use super::*;
    use proptest::prelude::*;

    const DIALECTS: [&str; 9] = [
        "common-lisp",
        "clojure",
        "scheme",
        "racket",
        "emacs-lisp",
        "fennel",
        "janet",
        "lispico-clojure",
        "lispico-cl",
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
                Just("\r\n".to_owned()),
                Just("\r".to_owned()),
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
        fn lossless_and_total(src in lisp_ish(), d in 0usize..9) {
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
        fn arbitrary_text_never_panics(src in "\\PC*", d in 0usize..9) {
            let t = parse(DIALECTS[d], &src);
            let joined: String = t.tokens().iter().map(|k| t.token_text(k)).collect();
            prop_assert_eq!(joined, src);
        }
    }
}
