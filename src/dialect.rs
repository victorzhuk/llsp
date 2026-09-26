use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;

use crate::config::merge_tables;

const BUILTIN: &[(&str, &str)] = &[
    ("common-lisp", include_str!("../dialects/common-lisp.toml")),
    ("clojure", include_str!("../dialects/clojure.toml")),
    ("scheme", include_str!("../dialects/scheme.toml")),
    ("racket", include_str!("../dialects/racket.toml")),
    ("emacs-lisp", include_str!("../dialects/emacs-lisp.toml")),
    ("fennel", include_str!("../dialects/fennel.toml")),
    ("janet", include_str!("../dialects/janet.toml")),
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dialect {
    #[serde(skip)]
    pub name: String,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub language_ids: Vec<String>,
    #[serde(default)]
    pub modeline_names: Vec<String>,
    #[serde(default = "yes")]
    pub case_sensitive: bool,
    #[serde(default)]
    pub namespace_separators: Vec<String>,
    #[serde(default)]
    pub keyword_prefixes: Vec<String>,
    pub reader: ReaderRules,
    #[serde(default)]
    pub defs: HashMap<String, DefSpec>,
    #[serde(default)]
    pub def_prefixes: Vec<String>,
    #[serde(default)]
    pub bindings: HashMap<String, BindingShape>,
    #[serde(default)]
    pub namespace_forms: Vec<String>,
    #[serde(default)]
    pub pattern_ignore: Vec<String>,
    #[serde(default)]
    pub indent: HashMap<String, u32>,
    #[serde(default)]
    pub indent_prefixes: HashMap<String, u32>,
    #[serde(default)]
    pub special_forms: Vec<String>,
    #[serde(default)]
    pub builtins: Vec<String>,
    #[serde(default)]
    pub constants: Vec<String>,
    #[serde(skip)]
    lookup: Lookup,
}

#[derive(Debug, Clone, Default)]
struct Lookup {
    defs: FxHashMap<String, DefSpec>,
    bindings: FxHashMap<String, BindingShape>,
    namespace_forms: FxHashSet<String>,
    pattern_ignore: FxHashSet<String>,
    indent: FxHashMap<String, u32>,
    special_forms: FxHashSet<String>,
    builtins: FxHashSet<String>,
    constants: FxHashSet<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReaderRules {
    #[serde(default = "parens")]
    pub brackets: Vec<String>,
    #[serde(default)]
    pub line_comment: String,
    #[serde(default)]
    pub block_comment: Option<(String, String)>,
    #[serde(default)]
    pub nested_block_comments: bool,
    #[serde(default)]
    pub datum_comments: Vec<String>,
    #[serde(default)]
    pub char_prefix: String,
    #[serde(default)]
    pub char_escape: bool,
    #[serde(default)]
    pub symbol_bars: bool,
    #[serde(default)]
    pub symbol_escape: bool,
    #[serde(default)]
    pub comma_whitespace: bool,
    #[serde(default)]
    pub long_string: String,
    #[serde(default)]
    pub terminators: String,
    #[serde(default)]
    pub sharp_dispatch: bool,
    #[serde(default)]
    pub prefixes: HashMap<String, u8>,
    #[serde(skip)]
    pub(crate) sorted_prefixes: Vec<(String, u8)>,
    #[serde(skip, default = "no_delimiters")]
    pub(crate) delimiters: [u8; 256],
    #[serde(skip, default = "no_delimiters")]
    pub(crate) closers: [u8; 256],
}

pub(crate) const OPEN: u8 = 1;
pub(crate) const CLOSE: u8 = 2;
pub(crate) const STOP: u8 = 4;
pub(crate) const SPECIAL: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SymbolKind {
    Function,
    Macro,
    Variable,
    Constant,
    Class,
    Struct,
    Type,
    Interface,
    Method,
    Module,
    Test,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefSpec {
    pub kind: SymbolKind,
    #[serde(default = "one")]
    pub name: usize,
    #[serde(default)]
    pub params: Option<Params>,
    #[serde(default)]
    pub doc: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum Params {
    At(usize),
    Search(ParamSearch),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParamSearch {
    /// First vector after the name, or arity lists `([x] body)`.
    Vector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingShape {
    /// `(let ((a 1) b) body)`; a symbol first means a named let.
    Let,
    /// `(let [a 1 b 2] body)`
    LetVector,
    /// `(lambda (params) body)`
    Lambda,
    /// `(fn name? [params] body)` or `(fn ([x] ..) ([x y] ..))`
    Fn,
    /// `(destructuring-bind pattern value body)`
    Bind,
    /// `(dolist (x list) body)`
    Single,
    /// `(flet ((name (params) body)) body)`
    Flet,
}

fn yes() -> bool {
    true
}

fn one() -> usize {
    1
}

fn no_delimiters() -> [u8; 256] {
    [0; 256]
}

fn parens() -> Vec<String> {
    vec!["()".into()]
}

impl Dialect {
    fn finish(&mut self) -> Result<()> {
        let r = &mut self.reader;
        let mut table = [0u8; 256];
        for b in b" \t\n\r\x0c\"" {
            table[*b as usize] |= STOP;
        }
        if r.comma_whitespace {
            table[b',' as usize] |= STOP;
        }
        for pair in &r.brackets {
            let &[open, close] = pair.as_bytes() else {
                bail!(
                    "dialect {}: bracket pair {pair:?} must be two ASCII chars",
                    self.name
                );
            };
            table[open as usize] |= OPEN | STOP;
            r.closers[open as usize] = close;
            table[close as usize] |= CLOSE | STOP;
        }
        for b in r.terminators.bytes() {
            table[b as usize] |= STOP;
        }
        if let Some(b) = r.line_comment.bytes().next() {
            table[b as usize] |= STOP;
        }
        let starts = r
            .prefixes
            .keys()
            .chain(&r.datum_comments)
            .chain(r.block_comment.iter().map(|(open, _)| open))
            .chain([&r.char_prefix, &r.long_string, &r.line_comment]);
        for s in starts {
            if let Some(b) = s.bytes().next() {
                table[b as usize] |= SPECIAL;
            }
        }
        if r.sharp_dispatch {
            table[b'#' as usize] |= SPECIAL;
        }
        r.delimiters = table;
        r.sorted_prefixes = r.prefixes.iter().map(|(k, v)| (k.clone(), *v)).collect();
        r.sorted_prefixes.retain(|(k, _)| !k.is_empty());
        r.sorted_prefixes
            .sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(&b.0)));
        self.namespace_separators
            .sort_by_key(|s| std::cmp::Reverse(s.len()));

        let key = |s: &String| self.normalize(s).into_owned();
        let set = |v: &Vec<String>| v.iter().map(key).collect::<FxHashSet<_>>();
        self.lookup = Lookup {
            defs: self.defs.iter().map(|(k, v)| (key(k), v.clone())).collect(),
            bindings: self.bindings.iter().map(|(k, v)| (key(k), *v)).collect(),
            namespace_forms: set(&self.namespace_forms),
            pattern_ignore: set(&self.pattern_ignore),
            indent: self.indent.iter().map(|(k, v)| (key(k), *v)).collect(),
            special_forms: set(&self.special_forms),
            builtins: set(&self.builtins),
            constants: set(&self.constants),
        };
        Ok(())
    }

    pub fn normalize<'a>(&self, name: &'a str) -> std::borrow::Cow<'a, str> {
        if self.case_sensitive || !name.bytes().any(|b| b.is_ascii_uppercase()) {
            name.into()
        } else {
            name.to_ascii_lowercase().into()
        }
    }

    pub fn def_spec(&self, head: &str) -> Option<&DefSpec> {
        self.lookup.defs.get(head)
    }

    pub fn binding_shape(&self, head: &str) -> Option<BindingShape> {
        self.lookup.bindings.get(head).copied()
    }

    pub fn is_namespace_form(&self, head: &str) -> bool {
        self.lookup.namespace_forms.contains(head)
    }

    pub fn is_pattern_ignored(&self, name: &str) -> bool {
        self.lookup.pattern_ignore.contains(name)
    }

    pub fn is_special_form(&self, name: &str) -> bool {
        self.lookup.special_forms.contains(name)
    }

    pub fn is_builtin(&self, name: &str) -> bool {
        self.lookup.builtins.contains(name)
    }

    pub fn is_constant(&self, name: &str) -> bool {
        self.lookup.constants.contains(name)
    }

    pub fn has_def_prefix(&self, head: &str) -> bool {
        self.def_prefixes
            .iter()
            .any(|p| head.len() > p.len() && head.starts_with(p.as_str()))
    }

    pub fn indent_spec(&self, head: &str) -> Option<u32> {
        self.lookup.indent.get(head).copied().or_else(|| {
            self.indent_prefixes
                .iter()
                .filter(|(p, _)| head.starts_with(p.as_str()))
                .max_by_key(|(p, _)| p.len())
                .map(|(_, n)| *n)
        })
    }

    pub fn is_keyword(&self, text: &str) -> bool {
        self.keyword_prefixes
            .iter()
            .any(|p| text.len() > p.len() && text.starts_with(p.as_str()))
    }

    /// Splits `pkg:sym`, `ns/sym` into qualifier and name, if qualified.
    pub fn split_qualified<'a>(&self, text: &'a str) -> (Option<&'a str>, &'a str) {
        if self.is_keyword(text) {
            return (None, text);
        }
        for sep in &self.namespace_separators {
            if let Some(i) = text.find(sep.as_str()) {
                let name = &text[i + sep.len()..];
                if i > 0 && !name.is_empty() {
                    return (Some(&text[..i]), name);
                }
            }
        }
        (None, text)
    }
}

#[derive(Debug, Clone)]
pub struct Dialects {
    list: Vec<Arc<Dialect>>,
}

impl Dialects {
    pub fn builtin() -> Self {
        Self::load(&toml::Table::new()).expect("built-in dialects are valid")
    }

    pub fn load(overrides: &toml::Table) -> Result<Self> {
        let mut raw: FxHashMap<String, toml::Table> = FxHashMap::default();
        for (name, src) in BUILTIN {
            let table: toml::Table =
                toml::from_str(src).with_context(|| format!("built-in dialect {name}"))?;
            raw.insert((*name).to_owned(), table);
        }
        for (name, value) in overrides {
            let toml::Value::Table(t) = value else {
                bail!("dialect {name}: expected a table");
            };
            match raw.get_mut(name) {
                Some(base) => merge_tables(base, t.clone()),
                None => {
                    raw.insert(name.clone(), t.clone());
                }
            }
        }

        let mut names: Vec<&String> = raw.keys().collect();
        names.sort();
        let mut list = Vec::with_capacity(names.len());
        for name in names {
            let table = resolve(&raw, name, &mut Vec::new())?;
            let mut d: Dialect = table
                .try_into()
                .with_context(|| format!("dialect {name}"))?;
            d.name = name.clone();
            d.finish()?;
            list.push(Arc::new(d));
        }
        Ok(Self { list })
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<Dialect>> {
        self.list.iter()
    }

    pub fn get(&self, name: &str) -> Option<&Arc<Dialect>> {
        self.list.iter().find(|d| d.name == name)
    }

    pub fn by_extension(&self, ext: &str) -> Option<&Arc<Dialect>> {
        self.list
            .iter()
            .find(|d| d.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
    }

    pub fn by_language_id(&self, id: &str) -> Option<&Arc<Dialect>> {
        self.list
            .iter()
            .find(|d| d.language_ids.iter().any(|l| l == id))
    }

    pub fn by_modeline(&self, mode: &str) -> Option<&Arc<Dialect>> {
        self.list.iter().find(|d| {
            d.modeline_names
                .iter()
                .any(|m| m.eq_ignore_ascii_case(mode))
        })
    }
}

fn resolve(
    raw: &FxHashMap<String, toml::Table>,
    name: &str,
    stack: &mut Vec<String>,
) -> Result<toml::Table> {
    if stack.iter().any(|s| s == name) {
        stack.push(name.to_owned());
        bail!("dialect extends cycle: {}", stack.join(" -> "));
    }
    let Some(table) = raw.get(name) else {
        bail!("unknown dialect {name}");
    };
    let mut table = table.clone();
    let Some(parent) = table.remove("extends") else {
        return Ok(table);
    };
    let Some(parent) = parent.as_str() else {
        bail!("dialect {name}: extends must be a string");
    };
    stack.push(name.to_owned());
    let mut base = resolve(raw, parent, stack)?;
    stack.pop();
    for key in ["extensions", "language_ids", "modeline_names"] {
        if !table.contains_key(key) {
            base.remove(key);
        }
    }
    merge_tables(&mut base, table);
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(src: &str) -> toml::Table {
        toml::from_str(src).unwrap()
    }

    #[test]
    fn loads_all_builtins() {
        let d = Dialects::builtin();
        for name in [
            "common-lisp",
            "clojure",
            "scheme",
            "racket",
            "emacs-lisp",
            "fennel",
            "janet",
        ] {
            let dialect = d.get(name).unwrap_or_else(|| panic!("{name}"));
            assert!(!dialect.extensions.is_empty(), "{name} has extensions");
        }
    }

    #[test]
    fn override_adds_def_form() {
        let d = Dialects::load(&table(
            r#"[clojure.defs]
defroute = { kind = "function", params = 2 }"#,
        ))
        .unwrap();
        let clj = d.get("clojure").unwrap();
        assert_eq!(clj.def_spec("defroute").unwrap().kind, SymbolKind::Function);
        assert!(clj.def_spec("defn").is_some(), "built-in defs kept");
    }

    #[test]
    fn extends_inherits_rules() {
        let d = Dialects::load(&table(
            r#"[lfe]
extends = "common-lisp"
extensions = ["lfe"]
case_sensitive = true"#,
        ))
        .unwrap();
        let lfe = d.by_extension("lfe").unwrap();
        assert_eq!(lfe.name, "lfe");
        assert!(lfe.case_sensitive);
        assert!(lfe.def_spec("defun").is_some());
        assert_eq!(d.by_extension("lisp").unwrap().name, "common-lisp");
    }

    #[test]
    fn extends_cycle_is_error() {
        let err = Dialects::load(&table(
            "[a]\nextends = \"b\"\n[a.reader]\n[b]\nextends = \"a\"\n[b.reader]",
        ))
        .unwrap_err();
        assert!(err.to_string().contains("cycle"), "{err}");
    }

    #[test]
    fn case_insensitive_lookup() {
        let d = Dialects::builtin();
        let cl = d.get("common-lisp").unwrap();
        assert_eq!(cl.normalize("DEFUN"), "defun");
        assert!(cl.def_spec(&cl.normalize("DEFUN")).is_some());
        let clj = d.get("clojure").unwrap();
        assert_eq!(clj.normalize("Foo"), "Foo");
    }

    #[test]
    fn qualified_names() {
        let d = Dialects::builtin();
        let cl = d.get("common-lisp").unwrap();
        assert_eq!(cl.split_qualified("pkg::foo"), (Some("pkg"), "foo"));
        assert_eq!(cl.split_qualified("pkg:foo"), (Some("pkg"), "foo"));
        assert_eq!(cl.split_qualified(":foo"), (None, ":foo"));
        let clj = d.get("clojure").unwrap();
        assert_eq!(clj.split_qualified("str/join"), (Some("str"), "join"));
        assert_eq!(clj.split_qualified("/"), (None, "/"));
    }
}
