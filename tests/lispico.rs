use llsp::config::{Diagnostics, Settings};
use llsp::diagnostics;
use llsp::dialect::{Cell, Dialect, Dialects};
use llsp::syntax::{ErrorKind, NodeKind, SyntaxError, Tree};

fn dialect(name: &str) -> Dialect {
    Dialects::builtin().get(name).unwrap().as_ref().clone()
}

fn tree(name: &str, src: &str) -> Tree {
    Tree::parse(src.to_owned(), &dialect(name))
}

#[test]
fn accepted_forms_read_clean() {
    let clojure = [
        "(def x 1)",
        "(defn f [x] (+ x 1))",
        "(defmacro m [x] (list x))",
        "(fn [x] x)",
        "(if a b c)",
        "(cond (odd? x) :odd :else :even)",
        "(when a b)",
        "(let [x 1] x)",
        "(let* [x 1 y x] y)",
        "(do a b)",
        "'x",
        "`(a b ~c ~@d)",
        "(set! x 2)",
        "(loop [x 0] (if (< x 3) (recur (inc x)) x))",
        "(try (throw e) (catch :error e))",
        "(and a b)",
        "(or a b)",
        "(not a)",
        "(-> x (assoc :a 1) str)",
        "(as-> x n (inc n))",
        "(if-let [x (f)] x nil)",
        "(when-let [x (f)] x)",
    ];
    for form in clojure {
        assert!(tree("lispico-clojure", form).errors().is_empty(), "{form}");
    }
    let cl = [
        "(defun f (x) (+ x 1))",
        "(defn f (x) x)",
        "(defmacro m (x) x)",
        "(setq x 2)",
        "(progn a b)",
        "(funcall f 1)",
        "(function f)",
        "(let ((x 1)) x)",
        "(let* ((x 1) (y x)) y)",
        "(loop ((x 0)) (if (< x 3) (recur (+ x 1)) x))",
        "(if-let ((x (f))) x nil)",
        "(when-let ((x (f))) x)",
        "(car xs)",
        "(first xs)",
        "(mapcar f xs)",
        "(append xs ys)",
        "(null xs)",
        "'x",
        "`(a b)",
        "(and a b)",
        "(or a b)",
        "(not a)",
        "a:b",
    ];
    for form in cl {
        assert!(tree("lispico-cl", form).errors().is_empty(), "{form}");
    }
}

#[test]
fn cl_brackets_are_invalid() {
    for (src, open, close) in [("(f [x])", 3, 5), ("(f {x})", 3, 5)] {
        let t = tree("lispico-cl", src);
        let errs = t.errors();
        let invalid = |at: u32| SyntaxError {
            kind: ErrorKind::InvalidSyntax,
            start: at,
            end: at + 1,
        };
        assert_eq!(errs, &[invalid(open), invalid(close)], "{src}");
        // outer list intact: one closed Paren under the root
        let root = llsp::syntax::Tree::ROOT;
        let kids = t.children(root);
        assert_eq!(kids.len(), 1, "{src}");
        let list = t.node(kids[0]);
        assert_eq!(
            list.kind,
            NodeKind::List(llsp::syntax::Delim::Paren),
            "{src}"
        );
        assert!(list.closed, "{src}");
    }
}

#[test]
fn cl_dispatch_allowed() {
    let t = tree("lispico-cl", "#'f");
    assert!(t.errors().is_empty(), "{:?}", t.errors());
    let root = llsp::syntax::Tree::ROOT;
    let prefix = t.children(root)[0];
    assert_eq!(t.node(prefix).kind, NodeKind::Prefix);
    assert_eq!(t.prefix_text(prefix), "#'");
    let form = t.children(prefix)[0];
    assert_eq!(t.atom(form), Some("f"));

    let t = tree("lispico-cl", "#(1 2)");
    assert!(t.errors().is_empty(), "{:?}", t.errors());
    let prefix = t.children(root)[0];
    assert_eq!(t.node(prefix).kind, NodeKind::Prefix);
    assert_eq!(t.prefix_text(prefix), "#");
    let form = t.children(prefix)[0];
    assert_eq!(
        t.node(form).kind,
        NodeKind::List(llsp::syntax::Delim::Paren)
    );
    assert!(t.node(form).closed);
}

#[test]
fn clojure_hash_is_invalid() {
    let t = tree("lispico-clojure", "#{1 2} (ok)");
    let errs = t.errors();
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert_eq!(errs[0].kind, ErrorKind::InvalidSyntax);
    assert_eq!((errs[0].start, errs[0].end), (0, 1));
    // the following forms still parse
    let kids = t.children(llsp::syntax::Tree::ROOT);
    assert_eq!(kids.len(), 3);
    assert_eq!(
        t.node(kids[1]).kind,
        NodeKind::List(llsp::syntax::Delim::Brace)
    );
    assert_eq!(
        t.node(kids[2]).kind,
        NodeKind::List(llsp::syntax::Delim::Paren)
    );

    // #'f and #(1 2) read as Atom `#` (invalid) then a quoted form
    let t = tree("lispico-clojure", "#'f");
    let errs = t.errors();
    assert_eq!(
        errs,
        [SyntaxError {
            kind: ErrorKind::InvalidSyntax,
            start: 0,
            end: 1
        }],
        "{errs:?}"
    );
    let kids = t.children(llsp::syntax::Tree::ROOT);
    assert_eq!(t.atom(kids[0]), Some("#"));
    let prefix = kids[1];
    assert_eq!(t.node(prefix).kind, NodeKind::Prefix);
    assert_eq!(t.prefix_text(prefix), "'");
    assert_eq!(t.atom(t.children(prefix)[0]), Some("f"));

    let t = tree("lispico-clojure", "#(1 2)");
    let errs = t.errors();
    assert_eq!(
        errs,
        [SyntaxError {
            kind: ErrorKind::InvalidSyntax,
            start: 0,
            end: 1
        }],
        "{errs:?}"
    );
    let kids = t.children(llsp::syntax::Tree::ROOT);
    assert_eq!(t.atom(kids[0]), Some("#"));
    assert_eq!(
        t.node(kids[1]).kind,
        NodeKind::List(llsp::syntax::Delim::Paren)
    );
}

#[test]
fn language_id_selects_lispico() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rules/a.clj");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "(+ 1 2)").unwrap();
    let settings = Settings::from_table(toml::Table::new()).unwrap();
    let d = settings.detect(Some(&path), Some("lispico-clojure"), "");
    assert_eq!(d.name, "lispico-clojure");
}

#[test]
fn cl_vocabulary() {
    let d = dialect("lispico-cl");
    for name in ["car", "first", "mapcar"] {
        assert!(d.is_builtin(name), "{name}");
    }
    for src in ["(car xs)", "(first xs)", "(mapcar f xs)"] {
        let t = tree("lispico-cl", src);
        let a = llsp::analysis::Analysis::new(&t, &d);
        let cfg = Diagnostics {
            unresolved_call: llsp::config::Level::Error,
            ..Diagnostics::default()
        };
        let diags = diagnostics::check(&t, &a, &d, &cfg, |_| false);
        assert!(
            !diags.iter().any(|x| x.code == "unresolved-call"),
            "{src}: {diags:?}"
        );
    }
}

#[test]
fn special_forms_follow_kernel() {
    for name in ["lispico-clojure", "lispico-cl"] {
        let d = dialect(name);
        assert!(d.is_special_form("if"), "{name}");
        assert!(d.is_special_form("recur"), "{name}");
    }
    let cl = dialect("lispico-cl");
    assert!(!cl.is_special_form("set!"));
    assert!(!cl.is_special_form("do"));
    for f in ["defun", "setq", "progn", "funcall", "function"] {
        assert!(cl.is_special_form(f), "{f}");
    }
    let clj = dialect("lispico-clojure");
    assert!(!clj.function_cells);
    assert!(cl.function_cells);
    for (name, form) in [("lispico-cl", "defun"), ("lispico-clojure", "defn")] {
        let d = dialect(name);
        let spec = d.def_spec(form).unwrap();
        assert_eq!(spec.cell, Cell::Function, "{name}");
    }
}
