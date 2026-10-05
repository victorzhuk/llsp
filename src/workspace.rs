use std::path::{Path, PathBuf};
use std::sync::Arc;

use line_index::LineIndex;
use lsp_types::Uri;
use rayon::prelude::*;
use rustc_hash::FxHashMap;

use crate::analysis::{Analysis, Def, Target};
use crate::config::Settings;
use crate::dialect::{Cell, Dialect};
use crate::document::path_to_uri;
use crate::syntax::Tree;

/// A global occurrence: the base-name range and the namespace it names through a
/// qualifier or a `:refer`, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    pub start: u32,
    pub end: u32,
    pub explicit: Option<String>,
    pub cell: Cell,
}

/// What the workspace needs to know about one file without keeping its tree.
#[derive(Debug)]
pub struct FileSummary {
    pub uri: Uri,
    pub dialect: Arc<Dialect>,
    pub defs: Vec<Def>,
    pub namespace: Option<String>,
    namespace_starts: Vec<(u32, String)>,
    pub lines: LineIndex,
    /// Global (non-local) occurrences by normalized name, including definition names.
    pub refs: FxHashMap<String, Vec<Ref>>,
    def_keys: FxHashMap<String, Vec<u32>>,
    /// Lowercased definition names, one per line, scanned sequentially by fuzzy search.
    search_names: String,
}

impl FileSummary {
    pub fn new(uri: Uri, dialect: Arc<Dialect>, tree: &Tree, analysis: &Analysis) -> Self {
        let mut refs: FxHashMap<String, Vec<Ref>> = FxHashMap::default();
        for o in &analysis.occurrences {
            if o.target == Target::Global {
                refs.entry(o.key.clone()).or_default().push(Ref {
                    start: o.start,
                    end: o.end,
                    explicit: analysis.explicit_namespace(&dialect, o),
                    cell: o.cell,
                });
            }
        }
        let mut def_keys: FxHashMap<String, Vec<u32>> = FxHashMap::default();
        for (i, d) in analysis.defs.iter().enumerate() {
            def_keys.entry(d.key.clone()).or_default().push(i as u32);
        }
        let search_names = analysis
            .defs
            .iter()
            .map(|d| d.name.to_lowercase().replace('\n', " "))
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            search_names,
            uri,
            dialect,
            defs: analysis.defs.clone(),
            namespace: analysis.namespace.clone(),
            namespace_starts: analysis.namespace_starts.clone(),
            lines: LineIndex::new(tree.text()),
            refs,
            def_keys,
        }
    }

    pub fn from_text(uri: Uri, dialect: Arc<Dialect>, text: String) -> Self {
        let tree = Tree::parse(text, &dialect);
        let analysis = Analysis::new(&tree, &dialect);
        Self::new(uri, dialect, &tree, &analysis)
    }

    /// Definitions whose names fuzzy-match a lowercase `query`, with their scores.
    pub fn search<'a>(&'a self, query: &'a str) -> impl Iterator<Item = (u8, &'a Def)> + 'a {
        let names = (!self.defs.is_empty()).then_some(self.search_names.split('\n'));
        names
            .into_iter()
            .flatten()
            .zip(&self.defs)
            .filter_map(move |(name, d)| Some((crate::search::score_lowercase(query, name)?, d)))
    }

    pub fn namespace_at(&self, offset: u32) -> Option<&str> {
        crate::analysis::namespace_at(&self.namespace_starts, offset)
    }

    pub fn defs_named<'a>(&'a self, key: &str) -> impl Iterator<Item = &'a Def> + 'a {
        self.def_keys
            .get(key)
            .into_iter()
            .flatten()
            .map(|&i| &self.defs[i as usize])
    }
}

#[derive(Debug, Default)]
pub struct Index {
    files: FxHashMap<Uri, Arc<FileSummary>>,
    by_def: FxHashMap<String, Vec<Arc<FileSummary>>>,
}

impl Index {
    pub fn insert(&mut self, summary: FileSummary) {
        let file = Arc::new(summary);
        for key in file.def_keys.keys() {
            self.by_def
                .entry(key.clone())
                .or_default()
                .push(file.clone());
        }
        if let Some(old) = self.files.insert(file.uri.clone(), file) {
            self.unlink(&old);
        }
    }

    pub fn remove(&mut self, uri: &Uri) {
        if let Some(old) = self.files.remove(uri) {
            self.unlink(&old);
        }
    }

    fn unlink(&mut self, old: &Arc<FileSummary>) {
        for key in old.def_keys.keys() {
            if let Some(files) = self.by_def.get_mut(key) {
                files.retain(|f| !Arc::ptr_eq(f, old));
                if files.is_empty() {
                    self.by_def.remove(key);
                }
            }
        }
    }

    pub fn get(&self, uri: &Uri) -> Option<&Arc<FileSummary>> {
        self.files.get(uri)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn files(&self) -> impl Iterator<Item = &Arc<FileSummary>> {
        self.files.values()
    }

    /// Definitions named `key` in any file; callers filter by dialect.
    pub fn defs_named<'a>(
        &'a self,
        key: &'a str,
    ) -> impl Iterator<Item = (&'a Arc<FileSummary>, &'a Def)> + 'a {
        self.by_def
            .get(key)
            .into_iter()
            .flatten()
            .flat_map(move |f| f.defs_named(key).map(move |d| (f, d)))
    }

    /// Definitions named `key` in files of `dialect`, whose cell satisfies `cell`.
    /// The one definition filter every feature and both front ends share.
    pub fn defs_in<'a>(
        &'a self,
        key: &'a str,
        dialect: &'a Dialect,
        cell: Cell,
    ) -> impl Iterator<Item = (&'a Arc<FileSummary>, &'a Def)> + 'a {
        self.defs_named(key).filter(move |(f, d)| {
            f.dialect.name == dialect.name && dialect.cells_match(d.cell, cell)
        })
    }

    pub fn refs_named<'a>(
        &'a self,
        key: &'a str,
    ) -> impl Iterator<Item = (&'a Arc<FileSummary>, &'a Ref)> + 'a {
        self.files
            .values()
            .flat_map(move |f| f.refs.get(key).into_iter().flatten().map(move |r| (f, r)))
    }
}

/// Walks `roots` for files whose dialect is known, honoring excludes and
/// `.gitignore`, up to `max_files` files in total across all roots.
pub fn discover(settings: &Settings, roots: &[PathBuf], max_files: usize) -> (Vec<PathBuf>, bool) {
    let cfg = &settings.config;
    let mut excludes = globset::GlobSetBuilder::new();
    for g in &cfg.workspace.exclude {
        match globset::Glob::new(g) {
            Ok(g) => {
                excludes.add(g);
            }
            Err(e) => log::warn!("workspace.exclude: {e}"),
        }
    }
    let excludes = excludes
        .build()
        .unwrap_or_else(|_| globset::GlobSet::empty());
    let mut out = Vec::new();
    let mut truncated = false;
    'roots: for root in roots {
        let walker = ignore::WalkBuilder::new(root)
            .follow_links(false)
            .require_git(false)
            .build();
        for entry in walker.flatten() {
            let path = entry.path();
            if !entry.file_type().is_some_and(|t| t.is_file()) || excludes.is_match(path) {
                continue;
            }
            let known = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| settings.dialects.by_extension(e).is_some())
                || settings.associations.iter().any(|(g, _)| g.is_match(path));
            if !known {
                continue;
            }
            if out.len() >= max_files {
                truncated = true;
                break 'roots;
            }
            out.push(path.to_path_buf());
        }
    }
    (out, truncated)
}

pub fn summarize(settings: &Settings, path: &Path) -> Option<FileSummary> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > settings.config.files.max_file_size {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    let uri = path_to_uri(path)?;
    let dialect = settings.detect(Some(path), None, &text);
    Some(FileSummary::from_text(uri, dialect, text))
}

pub fn scan(settings: &Settings, roots: &[PathBuf]) -> Vec<FileSummary> {
    let (files, truncated) = discover(settings, roots, settings.config.workspace.max_files);
    if truncated {
        log::warn!(
            "workspace has more than {} lisp files; indexing the first {}",
            settings.config.workspace.max_files,
            files.len()
        );
    }
    files
        .par_iter()
        .filter_map(|p| summarize(settings, p))
        .collect()
}

/// True when `path` resolves inside one of the canonical `roots`.
pub fn is_inside(roots: &[PathBuf], path: &Path) -> bool {
    let Ok(real) = path.canonicalize() else {
        return false;
    };
    roots.iter().any(|r| real.starts_with(r))
}

/// Declared indent hints for `dialect`, resolved across the whole index. Both
/// front ends — `llsp format` and `textDocument/formatting` — use this lookup so
/// they produce the same edits for the same workspace.
pub fn indent_hints<'a>(
    index: &'a Index,
    dialect: &'a Dialect,
) -> impl Fn(&str) -> Option<u32> + 'a {
    move |key| {
        index
            .defs_named(key)
            .filter(|(f, _)| f.dialect.name == dialect.name)
            .find_map(|(_, d)| d.indent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Layers;

    fn settings(toml: &str) -> Settings {
        Layers {
            cli: toml::from_str(toml).unwrap(),
            ..Layers::default()
        }
        .resolve()
        .unwrap()
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    fn names(files: &[PathBuf], root: &Path) -> Vec<String> {
        let mut v: Vec<_> = files
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        v.sort();
        v
    }

    #[test]
    fn discover_honors_excludes_and_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "src/a.lisp", "(defun a ())");
        write(root, "src/b.clj", "(defn b [])");
        write(root, "target/gen.lisp", "(defun gen ())");
        write(root, "ignored/x.lisp", "");
        write(root, ".gitignore", "ignored/\n");
        write(root, "notes.txt", "");
        let (files, truncated) = discover(&settings(""), &[root.to_path_buf()], usize::MAX);
        assert_eq!(names(&files, root), ["src/a.lisp", "src/b.clj"]);
        assert!(!truncated);
    }

    #[test]
    fn discover_limits_file_count() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            write(dir.path(), &format!("f{i}.scm"), "(define x 1)");
        }
        let s = settings("[workspace]\nmax_files = 3");
        let (files, truncated) = discover(
            &s,
            &[dir.path().to_path_buf()],
            s.config.workspace.max_files,
        );
        assert_eq!(files.len(), 3);
        assert!(truncated);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "secret.lisp", "(defun secret ())");
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.lisp", "");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link")).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.lisp"),
            dir.path().join("direct.lisp"),
        )
        .unwrap();
        let s = settings("");
        let (files, _) = discover(&s, &[dir.path().to_path_buf()], usize::MAX);
        let summaries: Vec<_> = files.iter().filter_map(|p| summarize(&s, p)).collect();
        assert_eq!(summaries.len(), 1, "{files:?}");
        let root = dir.path().canonicalize().unwrap();
        assert!(!is_inside(
            std::slice::from_ref(&root),
            &dir.path().join("link/secret.lisp")
        ));
        assert!(is_inside(&[root], &dir.path().join("a.lisp")));
    }

    #[test]
    fn index_replace_and_remove_update_names() {
        let s = settings("");
        let d = s.dialects.get("common-lisp").unwrap().clone();
        let uri = |n: &str| path_to_uri(Path::new(n)).unwrap();
        let file = |n: &str, text: &str| FileSummary::from_text(uri(n), d.clone(), text.into());
        let mut index = Index::default();
        index.insert(file("/a.lisp", "(defun f ()) (defun g ())"));
        index.insert(file("/b.lisp", "(defun f ())"));
        assert_eq!(index.defs_named("f").count(), 2);

        index.insert(file("/a.lisp", "(defun h ())"));
        assert_eq!(index.defs_named("f").count(), 1);
        assert_eq!(index.defs_named("g").count(), 0);
        assert_eq!(index.defs_named("h").count(), 1);

        index.remove(&uri("/b.lisp"));
        index.remove(&uri("/missing.lisp"));
        assert_eq!(index.defs_named("f").count(), 0);
        assert!(index.by_def.keys().eq(["h"]));
    }

    #[test]
    fn scan_and_query() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.lisp", "(defun helper (x) x)\n(helper 1)");
        write(
            dir.path(),
            "b.lisp",
            "(defun main () (helper 2) (let ((helper 3)) helper))",
        );
        write(dir.path(), "big.lisp", &"(a)".repeat(100));
        let s = settings("[files]\nmax_file_size = 200");
        let mut index = Index::default();
        for f in scan(&s, &[dir.path().to_path_buf()]) {
            index.insert(f);
        }
        assert_eq!(index.len(), 2, "big file skipped");
        assert_eq!(index.defs_named("helper").count(), 1);
        assert_eq!(
            index.refs_named("helper").count(),
            3,
            "local binding excluded"
        );
        assert_eq!(
            index.defs_named("main").next().unwrap().1.kind,
            crate::dialect::SymbolKind::Function
        );
    }
}
