use super::*;
use crate::dialect::Dialects;

fn analyze(dialect: &str, src: &str) -> (Tree, Analysis) {
    let ds = Dialects::builtin();
    let d = ds.get(dialect).unwrap();
    let t = Tree::parse(src.to_owned(), d);
    let a = Analysis::new(&t, d);
    (t, a)
}

fn def<'a>(a: &'a Analysis, name: &str) -> &'a Def {
    a.defs.iter().find(|d| d.name == name).unwrap_or_else(|| {
        panic!(
            "no def {name}: {:?}",
            a.defs.iter().map(|d| &d.name).collect::<Vec<_>>()
        )
    })
}

/// Target of the n-th occurrence of `name` (0-based), as (binder start) or None for global.
fn target(src: &str, a: &Analysis, name: &str, n: usize) -> Option<u32> {
    let occ = a
        .occurrences
        .iter()
        .filter(|o| &src[o.start as usize..o.end as usize] == name)
        .nth(n)
        .unwrap_or_else(|| panic!("occurrence {n} of {name}"));
    match occ.target {
        Target::Local(b) => Some(a.binders[b as usize].start),
        Target::Global => None,
    }
}

fn nth(src: &str, pat: &str, n: usize) -> u32 {
    src.match_indices(pat).nth(n).unwrap().0 as u32
}

#[test]
fn common_lisp_defun() {
    let (_, a) = analyze("common-lisp", "(defun area (w h) \"Area.\" (* w h))");
    let d = def(&a, "area");
    assert_eq!(d.kind, SymbolKind::Function);
    assert_eq!(
        d.signatures,
        vec![Signature {
            label: "(w h)".into(),
            params: vec!["w".into(), "h".into()]
        }]
    );
    assert_eq!(d.doc.as_deref(), Some("Area."));
    assert_eq!((d.name_start, d.name_end), (7, 11));
}

#[test]
fn clojure_multi_arity() {
    let src = "(defn f \"doc\" ([x] x) ([x y] y))";
    let (_, a) = analyze("clojure", src);
    let d = def(&a, "f");
    let labels: Vec<_> = d.signatures.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["[x]", "[x y]"]);
    assert_eq!(d.doc.as_deref(), Some("doc"));
    assert_eq!(target(src, &a, "y", 1), Some(nth(src, "y", 0)));
    assert_eq!(target(src, &a, "x", 1), Some(nth(src, "x", 0)));
    assert_eq!(target(src, &a, "x", 2), Some(nth(src, "x", 2)));
}

#[test]
fn scheme_define() {
    let src = "(define (sq x) (* x x)) (define n 10) (define g (lambda (y) y))";
    let (_, a) = analyze("scheme", src);
    let sq = def(&a, "sq");
    assert_eq!(sq.kind, SymbolKind::Function);
    assert_eq!(sq.signatures[0].params, ["x"]);
    assert_eq!(target(src, &a, "x", 1), Some(nth(src, "x", 0)));
    assert_eq!(def(&a, "n").kind, SymbolKind::Variable);
    assert_eq!(def(&a, "g").kind, SymbolKind::Function);
}

#[test]
fn setf_function_name() {
    let (_, a) = analyze("common-lisp", "(defun (setf foo) (v x) v)");
    let d = def(&a, "(setf foo)");
    assert_eq!(d.signatures[0].label, "(v x)");
    assert_eq!((d.name_start, d.name_end), (7, 17));
}

#[test]
fn metadata_name_and_defvar_doc() {
    let (_, a) = analyze(
        "clojure",
        "(defn ^:private helper [a] a) (def ^{:doc \"x\"} v 1)",
    );
    assert_eq!(def(&a, "helper").signatures[0].label, "[a]");
    assert_eq!(def(&a, "v").kind, SymbolKind::Variable);
    let (_, a) = analyze("common-lisp", "(defvar *x* 10 \"The x.\")");
    assert_eq!(def(&a, "*x*").doc.as_deref(), Some("The x."));
    assert_eq!(def(&a, "*x*").kind, SymbolKind::Variable);
}

#[test]
fn auto_def_prefix() {
    let (_, a) = analyze(
        "common-lisp",
        "(deftest my-test () (is t)) (defroute-macro r)",
    );
    assert_eq!(def(&a, "my-test").kind, SymbolKind::Function);
    assert_eq!(def(&a, "r").kind, SymbolKind::Macro);
    let (_, a) = analyze("common-lisp", "(define-key 'x)");
    assert!(a.defs.is_empty(), "quoted name is not a symbol");
}

#[test]
fn in_package_namespace() {
    let (_, a) = analyze("common-lisp", "(in-package :app)\n(defun run ())");
    assert_eq!(def(&a, "run").namespace.as_deref(), Some("app"));
    assert_eq!(a.namespace.as_deref(), Some("app"));
    let (_, a) = analyze("common-lisp", "(in-package #:APP)\n(defun run ())");
    assert_eq!(def(&a, "run").namespace.as_deref(), Some("app"));
}

#[test]
fn clojure_ns_and_alias() {
    let (_, a) = analyze(
        "clojure",
        "(ns a.core (:require [clojure.string :as str] [b.c :as-alias bc]))\n(defn f [] (str/join []))",
    );
    assert_eq!(
        a.aliases.get("str").map(String::as_str),
        Some("clojure.string")
    );
    assert_eq!(a.aliases.get("bc").map(String::as_str), Some("b.c"));
    assert_eq!(def(&a, "f").namespace.as_deref(), Some("a.core"));
    assert_eq!(def(&a, "a.core").kind, SymbolKind::Module);
    let join = a.occurrences.iter().find(|o| o.key == "join").unwrap();
    assert_eq!(join.qualifier.as_deref(), Some("str"));
    assert_eq!(a.resolve_qualifier("str"), "clojure.string");
}

#[test]
fn refers_and_namespace_switches() {
    let src = "(ns a.core (:require [b.c :as c :refer [run Stop]] [d.e :refer :all]))\n(run)\n(in-ns 'x.y)\n(c/run)";
    let (_, a) = analyze("clojure", src);
    assert_eq!(a.refers.get("run").map(String::as_str), Some("b.c"));
    assert_eq!(a.refers.get("Stop").map(String::as_str), Some("b.c"));
    assert_eq!(a.refers.len(), 2);
    assert_eq!(a.aliases.get("c").map(String::as_str), Some("b.c"));
    assert_eq!(a.namespace_at(0), Some("a.core"));
    assert_eq!(a.namespace_at(nth(src, "(run)", 0)), Some("a.core"));
    assert_eq!(a.namespace_at(nth(src, "c/run", 0)), Some("x.y"));

    let (_, a) = analyze("common-lisp", "(defun f ()) (in-package :app) (defun g ())");
    assert_eq!(a.namespace_at(1), None);
    assert_eq!(a.namespace_at(20), Some("app"));
}

#[test]
fn shadowing() {
    let src = "(let ((x 1)) (let ((x 2)) x) x)";
    let (_, a) = analyze("common-lisp", src);
    assert_eq!(target(src, &a, "x", 2), Some(nth(src, "x", 1)));
    assert_eq!(target(src, &a, "x", 3), Some(nth(src, "x", 0)));
}

#[test]
fn init_does_not_see_own_binder() {
    let src = "(let ((x (f x))) x)";
    let (_, a) = analyze("common-lisp", src);
    assert_eq!(target(src, &a, "x", 1), None);
    assert_eq!(target(src, &a, "x", 2), Some(nth(src, "x", 0)));
}

#[test]
fn clojure_destructuring() {
    let src = "(let [{:keys [a b] :as m} v] (+ a b m))";
    let (_, a) = analyze("clojure", src);
    let names: Vec<_> = a.binders.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "m"]);
    assert_eq!(target(src, &a, "a", 1), Some(nth(src, "a", 0)));
    assert_eq!(target(src, &a, "m", 1), Some(nth(src, "m", 0)));
    assert_eq!(target(src, &a, "v", 0), None);
}

#[test]
fn case_lambda_clauses() {
    let src = "(case-lambda ((x) x) ((x y) y))";
    let (_, a) = analyze("scheme", src);
    assert_eq!(target(src, &a, "x", 1), Some(nth(src, "x", 0)));
    assert_eq!(target(src, &a, "x", 2), Some(nth(src, "x", 2)));
    assert_eq!(target(src, &a, "y", 1), Some(nth(src, "y", 0)));
}

#[test]
fn defmethod_params() {
    let src = "(defmethod area :around ((s square)) (side s))";
    let (_, a) = analyze("common-lisp", src);
    assert_eq!(target(src, &a, "s", 1), Some(nth(src, "(s", 0) + 1));
    assert_eq!(def(&a, "area").kind, SymbolKind::Method);
}

#[test]
fn lambda_list_markers() {
    let src = "(defun f (a &optional (b *d*) &key c) (list a b c *d*))";
    let (_, a) = analyze("common-lisp", src);
    let names: Vec<_> = a.binders.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["a", "b", "c"]);
    assert_eq!(target(src, &a, "*d*", 1), None);
}

#[test]
fn flet_and_named_let_and_fn() {
    let src = "(flet ((g (y) y)) (g 1))";
    let (_, a) = analyze("common-lisp", src);
    assert_eq!(target(src, &a, "g", 1), Some(nth(src, "g", 0)));
    assert_eq!(target(src, &a, "y", 1), Some(nth(src, "y", 0)));

    let src = "(let loop ((i 0)) (loop (+ i 1)))";
    let (_, a) = analyze("scheme", src);
    assert_eq!(target(src, &a, "loop", 1), Some(nth(src, "loop", 0)));
    assert_eq!(target(src, &a, "i", 1), Some(nth(src, "i", 0)));

    let src = "(fn [x] #(+ x $1))";
    let (_, a) = analyze("fennel", src);
    assert_eq!(target(src, &a, "x", 1), Some(nth(src, "x", 0)));

    let src = "(each [k v (pairs t)] (print k v))";
    let (_, a) = analyze("fennel", src);
    let names: Vec<_> = a.binders.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["k", "v"]);
    assert_eq!(target(src, &a, "v", 1), Some(nth(src, "v", 0)));
}

#[test]
fn janet_and_elisp_bindings() {
    let src = "(defn f [a &opt b] (each x a (print x b)))";
    let (_, a) = analyze("janet", src);
    assert_eq!(target(src, &a, "b", 1), Some(nth(src, "b", 0)));
    assert_eq!(target(src, &a, "x", 1), Some(nth(src, "x", 0)));

    let src = "(condition-case err (foo) (error (message err)))";
    let (_, a) = analyze("emacs-lisp", src);
    assert_eq!(target(src, &a, "err", 1), Some(nth(src, "err", 0)));
}

#[test]
fn occurrence_filtering() {
    let (_, a) = analyze(
        "clojure",
        "(f :k 1 -2 nil true \"s\" \\c #_ (g) ::kw 1.5 x)",
    );
    let keys: Vec<_> = a.occurrences.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, ["f", "x"]);
    let (_, a) = analyze("common-lisp", "(1+ #:foo t nil)");
    let keys: Vec<_> = a.occurrences.iter().map(|o| o.key.as_str()).collect();
    assert_eq!(keys, ["1+"]);
}

#[test]
fn case_folding() {
    let (_, a) = analyze("common-lisp", "(defun foo ()) (FOO) (Foo)");
    assert!(a.occurrences.iter().filter(|o| o.key == "foo").count() == 3);
    assert_eq!(def(&a, "foo").key, "foo");
    let (_, a) = analyze("emacs-lisp", "(defun foo ()) (FOO)");
    assert!(a.occurrences.iter().any(|o| o.key == "FOO"));
}

#[test]
fn qualified_occurrences() {
    let src = "(pkg::helper x)";
    let (_, a) = analyze("common-lisp", src);
    let o = &a.occurrences[0];
    assert_eq!(
        (o.key.as_str(), o.qualifier.as_deref()),
        ("helper", Some("pkg"))
    );
    assert_eq!(&src[o.start as usize..o.end as usize], "helper");
}

#[test]
fn indent_hints() {
    let (_, a) = analyze(
        "emacs-lisp",
        "(defmacro with-x (a &rest body) (declare (indent 1)) body)",
    );
    assert_eq!(def(&a, "with-x").indent, Some(1));
    let (_, a) = analyze(
        "clojure",
        "(defmacro my-let {:style/indent 1} [b & body] body)",
    );
    assert_eq!(def(&a, "my-let").indent, Some(1));
    let (_, a) = analyze(
        "clojure",
        "(defmacro ^{:style/indent 2} m [a b & body] body)",
    );
    assert_eq!(def(&a, "m").indent, Some(2));
}

#[test]
fn lookups_by_offset() {
    let src = "(defun f (x) (g x))";
    let (_, a) = analyze("common-lisp", src);
    assert_eq!(a.def_at(8).unwrap().name, "f");
    assert_eq!(a.occurrence_at(14).unwrap().key, "g");
    assert_eq!(a.occurrence_at(15).unwrap().key, "g");
    assert_eq!(a.occurrence_at(16).unwrap().key, "x");
    let visible: Vec<_> = a.visible_binders(16).map(|b| b.name.as_str()).collect();
    assert_eq!(visible, ["x"]);
    assert!(a.visible_binders(3).next().is_none());
}
