use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use line_index::{LineIndex, TextSize, WideEncoding, WideLineCol};
use lsp_types::{Position, Range, TextDocumentContentChangeEvent, Uri};
use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};

use crate::analysis::Analysis;
use crate::config::Settings;
use crate::dialect::Dialect;
use crate::syntax::Tree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16,
}

#[derive(Debug, Clone)]
pub struct Document {
    pub version: i32,
    /// URI as the client spelled it, for messages sent back.
    pub client_uri: Uri,
    pub dialect: Arc<Dialect>,
    tree: Tree,
    lines: LineIndex,
    analysis: Analysis,
    max_size: u64,
}

impl Document {
    /// Texts over `max_size` bytes are kept but not read or analyzed.
    pub fn new(text: String, version: i32, dialect: Arc<Dialect>, max_size: u64) -> Self {
        let lines = LineIndex::new(&text);
        let mut doc = Self {
            version,
            client_uri: Uri::from_str("untitled:llsp").expect("valid URI"),
            dialect,
            tree: Tree::unparsed(String::new()),
            lines,
            analysis: Analysis::default(),
            max_size,
        };
        doc.reparse(text);
        doc
    }

    fn reparse(&mut self, text: String) {
        if self.oversized_text(&text) {
            self.tree = Tree::unparsed(text);
            self.analysis = Analysis::default();
        } else {
            self.tree = Tree::parse(text, &self.dialect);
            self.analysis = Analysis::new(&self.tree, &self.dialect);
        }
    }

    fn oversized_text(&self, text: &str) -> bool {
        text.len() as u64 > self.max_size
    }

    pub fn oversized(&self) -> bool {
        self.oversized_text(self.text())
    }

    pub fn text(&self) -> &str {
        self.tree.text()
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    pub fn lines(&self) -> &LineIndex {
        &self.lines
    }

    pub fn apply_changes(
        &mut self,
        changes: Vec<TextDocumentContentChangeEvent>,
        version: i32,
        enc: Encoding,
    ) {
        let mut text = std::mem::replace(&mut self.tree, Tree::unparsed(String::new())).into_text();
        for change in changes {
            match change.range {
                Some(range) => {
                    let start = to_offset(&self.lines, &text, range.start, enc) as usize;
                    let end = to_offset(&self.lines, &text, range.end, enc) as usize;
                    text.replace_range(start..end.max(start), &change.text);
                }
                None => text = change.text,
            }
            self.lines = LineIndex::new(&text);
        }
        self.version = version;
        self.reparse(text);
    }

    pub fn analysis(&self) -> &Analysis {
        &self.analysis
    }

    pub fn offset(&self, pos: Position, enc: Encoding) -> u32 {
        to_offset(&self.lines, self.text(), pos, enc)
    }

    pub fn position(&self, offset: u32, enc: Encoding) -> Position {
        to_position(&self.lines, self.text(), offset, enc)
    }

    pub fn range(&self, start: u32, end: u32, enc: Encoding) -> Range {
        Range::new(self.position(start, enc), self.position(end, enc))
    }
}

pub fn to_offset(lines: &LineIndex, text: &str, pos: Position, enc: Encoding) -> u32 {
    let Some(line) = lines.line(pos.line) else {
        return text.len() as u32;
    };
    let col = match enc {
        Encoding::Utf8 => Some(pos.character),
        Encoding::Utf16 => lines
            .to_utf8(
                WideEncoding::Utf16,
                WideLineCol {
                    line: pos.line,
                    col: pos.character,
                },
            )
            .map(|lc| lc.col),
    }
    .unwrap_or(u32::MAX);
    let start = u32::from(line.start()) as usize;
    let line_text = &text[start..u32::from(line.end()) as usize];
    let content = line_text.trim_end_matches(['\n', '\r']).len();
    let mut off = start + (col as usize).min(content);
    while !text.is_char_boundary(off) {
        off -= 1;
    }
    off as u32
}

pub fn to_position(lines: &LineIndex, text: &str, offset: u32, enc: Encoding) -> Position {
    let mut off = (offset as usize).min(text.len());
    while !text.is_char_boundary(off) {
        off -= 1;
    }
    lines_position(lines, off as u32, enc)
}

/// Converts an offset known to be on a char boundary, without the text.
pub fn lines_position(lines: &LineIndex, offset: u32, enc: Encoding) -> Position {
    let offset = TextSize::from(offset).min(lines.len());
    let Some(lc) = lines.try_line_col(offset) else {
        return Position::new(0, 0);
    };
    let col = match enc {
        Encoding::Utf8 => lc.col,
        Encoding::Utf16 => lines
            .to_wide(WideEncoding::Utf16, lc)
            .map_or(lc.col, |w| w.col),
    };
    Position::new(lc.line, col)
}

const PATH_SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let rest = uri.as_str().strip_prefix("file://")?;
    let path = &rest[rest.find('/')?..];
    let path = path.split(['?', '#']).next()?;
    let decoded = percent_decode_str(path).decode_utf8().ok()?;
    let bytes = decoded.as_bytes();
    let windows_drive = bytes.len() >= 3 && bytes[2] == b':' && bytes[1].is_ascii_alphabetic();
    Some(PathBuf::from(if windows_drive {
        &decoded[1..]
    } else {
        &decoded[..]
    }))
}

/// Canonical form of a `file:` URI so editor and walker spellings compare equal.
pub fn normalize_uri(uri: &Uri) -> Uri {
    uri_to_path(uri)
        .and_then(|p| path_to_uri(&p))
        .unwrap_or_else(|| uri.clone())
}

pub fn path_to_uri(path: &Path) -> Option<Uri> {
    let s = path.to_str()?.replace('\\', "/");
    let s = if s.starts_with('/') {
        s
    } else {
        format!("/{s}")
    };
    Uri::from_str(&format!("file://{}", utf8_percent_encode(&s, PATH_SEGMENT))).ok()
}

impl Settings {
    /// Picks a dialect: associations, client language id, `#lang`/modeline,
    /// extension, then the configured default.
    pub fn detect(
        &self,
        path: Option<&Path>,
        language_id: Option<&str>,
        text: &str,
    ) -> Arc<Dialect> {
        let d = &self.dialects;
        let by_assoc = || {
            let path = path?;
            let (_, name) = self.associations.iter().find(|(g, _)| g.is_match(path))?;
            d.get(name)
        };
        let by_first_line = || {
            let line = text.lines().next()?;
            if let Some(lang) = line.strip_prefix("#lang ") {
                let lang = lang.trim().split('/').next()?;
                return d.by_modeline(lang).or_else(|| d.by_modeline("racket"));
            }
            let (_, rest) = line.split_once("-*-")?;
            let (inner, _) = rest.split_once("-*-")?;
            let mode = inner
                .split(';')
                .find_map(|kv| kv.trim().strip_prefix("mode:"))
                .unwrap_or(inner)
                .trim();
            d.by_modeline(mode)
        };
        by_assoc()
            .or_else(|| language_id.and_then(|id| d.by_language_id(id)))
            .or_else(by_first_line)
            .or_else(|| d.by_extension(path?.extension()?.to_str()?))
            .or_else(|| d.get(&self.config.files.default_dialect))
            .or_else(|| d.iter().next())
            .cloned()
            .expect("at least one dialect")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Layers;

    fn doc(text: &str) -> Document {
        let d = crate::dialect::Dialects::builtin();
        Document::new(
            text.into(),
            0,
            d.get("common-lisp").unwrap().clone(),
            u64::MAX,
        )
    }

    fn change(
        range: Option<((u32, u32), (u32, u32))>,
        text: &str,
    ) -> TextDocumentContentChangeEvent {
        TextDocumentContentChangeEvent {
            range: range
                .map(|((a, b), (c, d))| Range::new(Position::new(a, b), Position::new(c, d))),
            range_length: None,
            text: text.into(),
        }
    }

    #[test]
    fn ranged_edit() {
        let mut d = doc("(a b)");
        d.apply_changes(
            vec![change(Some(((0, 3), (0, 4))), "c d")],
            2,
            Encoding::Utf16,
        );
        assert_eq!(d.text(), "(a c d)");
        assert_eq!(d.version, 2);
        assert_eq!(d.tree().children(Tree::ROOT).len(), 1);
    }

    #[test]
    fn sequential_edits_and_full_replace() {
        let mut d = doc("x\ny\n");
        d.apply_changes(
            vec![
                change(Some(((1, 0), (1, 1))), "(z)"),
                change(Some(((0, 0), (0, 0))), ";; c\n"),
            ],
            1,
            Encoding::Utf8,
        );
        assert_eq!(d.text(), ";; c\nx\n(z)\n");
        d.apply_changes(vec![change(None, "new")], 2, Encoding::Utf8);
        assert_eq!(d.text(), "new");
    }

    #[test]
    fn oversized_is_kept_unparsed() {
        let d = crate::dialect::Dialects::builtin();
        let cl = d.get("common-lisp").unwrap().clone();
        let mut doc = Document::new("(defun f ())".into(), 0, cl, 5);
        assert!(doc.oversized());
        assert_eq!(doc.text(), "(defun f ())");
        assert!(doc.analysis().defs.is_empty());
        doc.apply_changes(vec![change(None, "(a)")], 1, Encoding::Utf8);
        assert!(!doc.oversized());
        assert_eq!(doc.tree().children(Tree::ROOT).len(), 1);
    }

    #[test]
    fn out_of_range_clamps() {
        let mut d = doc("ab\ncd");
        d.apply_changes(
            vec![change(Some(((0, 99), (7, 3))), "!")],
            1,
            Encoding::Utf16,
        );
        assert_eq!(d.text(), "ab!");
    }

    #[test]
    fn utf16_positions() {
        let d = doc("(ö 😀 x)");
        let x = d.text().find('x').unwrap() as u32;
        assert_eq!(d.position(x, Encoding::Utf16), Position::new(0, 6));
        assert_eq!(d.position(x, Encoding::Utf8), Position::new(0, 9));
        assert_eq!(d.offset(Position::new(0, 6), Encoding::Utf16), x);
        assert_eq!(d.offset(Position::new(0, 9), Encoding::Utf8), x);
        let inside_emoji = d.offset(Position::new(0, 5), Encoding::Utf8);
        assert!(d.text().is_char_boundary(inside_emoji as usize));
    }

    #[test]
    fn uri_roundtrip() {
        let uri = Uri::from_str("file:///home/me/my%20code/a%23b.lisp").unwrap();
        let path = uri_to_path(&uri).unwrap();
        assert_eq!(path, PathBuf::from("/home/me/my code/a#b.lisp"));
        assert_eq!(path_to_uri(&path).unwrap(), uri);
        let win = Uri::from_str("file:///C:/src/a.el").unwrap();
        assert_eq!(uri_to_path(&win).unwrap(), PathBuf::from("C:/src/a.el"));
        assert!(uri_to_path(&Uri::from_str("untitled:Untitled-1").unwrap()).is_none());
    }

    #[test]
    fn detection_order() {
        let layers = Layers {
            project: toml::from_str("[files.associations]\n\"*.lsp\" = \"emacs-lisp\"").unwrap(),
            ..Layers::default()
        };
        let s = layers.resolve().unwrap();
        let name = |p: Option<&str>, id: Option<&str>, text: &str| {
            s.detect(p.map(Path::new), id, text).name.clone()
        };
        assert_eq!(name(Some("/a.lsp"), Some("lisp"), ""), "emacs-lisp");
        assert_eq!(name(Some("/a.lisp"), Some("clojure"), ""), "clojure");
        assert_eq!(
            name(Some("/script"), Some("x"), ";; -*- mode: clojure -*-\n"),
            "clojure"
        );
        assert_eq!(name(Some("/script"), None, ";; -*- Scheme -*-\n"), "scheme");
        assert_eq!(name(Some("/m"), None, "#lang racket/base\n"), "racket");
        assert_eq!(name(Some("/a.fnl"), None, ""), "fennel");
        assert_eq!(name(Some("/a.unknown"), None, ""), "common-lisp");
        assert_eq!(name(None, None, ""), "common-lisp");
    }
}
