use super::{ErrorKind, SyntaxError, Token, TokenKind};
use crate::dialect::{CLOSE, OPEN, ReaderRules, SPECIAL, STOP};

pub fn lex(text: &str, rules: &ReaderRules) -> (Vec<Token>, Vec<SyntaxError>) {
    let mut lx = Lexer {
        src: text.as_bytes(),
        text,
        rules,
        pos: 0,
        tokens: Vec::with_capacity(text.len() / 3),
        errors: Vec::new(),
    };
    lx.run();
    (lx.tokens, lx.errors)
}

struct Lexer<'a> {
    src: &'a [u8],
    text: &'a str,
    rules: &'a ReaderRules,
    pos: usize,
    tokens: Vec<Token>,
    errors: Vec<SyntaxError>,
}

impl Lexer<'_> {
    fn run(&mut self) {
        if self.src.starts_with(b"#!/") || self.src.starts_with(b"#! ") {
            let end = self.line_end(0);
            self.push(TokenKind::LineComment, 0, end);
        }
        while self.pos < self.src.len() {
            let start = self.pos;
            let kind = self.next_kind();
            debug_assert!(self.pos > start, "lexer must advance");
            self.push(kind, start, self.pos);
        }
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.pos = end;
        self.tokens.push(Token {
            kind,
            start: start as u32,
            end: end as u32,
        });
    }

    fn rest(&self) -> &[u8] {
        &self.src[self.pos..]
    }

    fn starts_with(&self, s: &str) -> bool {
        !s.is_empty() && self.rest().starts_with(s.as_bytes())
    }

    fn class(&self, b: u8) -> u8 {
        self.rules.delimiters[b as usize]
    }

    fn line_end(&self, from: usize) -> usize {
        self.src[from..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(self.src.len(), |i| from + i)
    }

    fn char_len(&self, at: usize) -> usize {
        self.text[at..].chars().next().map_or(1, char::len_utf8)
    }

    fn error(&mut self, kind: ErrorKind, start: usize, end: usize) {
        self.errors.push(SyntaxError {
            kind,
            start: start as u32,
            end: end as u32,
        });
    }

    fn next_kind(&mut self) -> TokenKind {
        let b = self.src[self.pos];
        let r = self.rules;
        let class = self.class(b);
        if class & (SPECIAL | STOP) == 0 {
            self.atom();
            return TokenKind::Atom;
        }
        if class & OPEN != 0 {
            self.pos += 1;
            return TokenKind::Open;
        }
        if class & CLOSE != 0 {
            self.pos += 1;
            return TokenKind::Close;
        }
        if is_space(b) || (b == b',' && r.comma_whitespace) {
            self.pos += self
                .rest()
                .iter()
                .position(|&c| !(is_space(c) || (c == b',' && r.comma_whitespace)))
                .unwrap_or(self.rest().len());
            return TokenKind::Whitespace;
        }
        if self.starts_with(&r.line_comment) {
            self.pos = self.line_end(self.pos);
            return TokenKind::LineComment;
        }
        if let Some((open, close)) = &r.block_comment
            && self.starts_with(open)
        {
            self.block_comment(open, close);
            return TokenKind::BlockComment;
        }
        if let Some(d) = r.datum_comments.iter().find(|d| self.starts_with(d)) {
            self.pos += d.len();
            return TokenKind::DatumComment;
        }
        if self.starts_with(&r.long_string) {
            self.long_string();
            return TokenKind::String;
        }
        if b == b'"' {
            self.string(self.pos);
            return TokenKind::String;
        }
        if self.starts_with(&r.char_prefix) && self.pos + r.char_prefix.len() < self.src.len() {
            self.char_literal();
            return TokenKind::Char;
        }
        if let Some((p, _)) = r.sorted_prefixes.iter().find(|(p, _)| self.starts_with(p)) {
            self.pos += p.len();
            return TokenKind::Prefix;
        }
        if b == b'#' && r.sharp_dispatch {
            return self.sharp();
        }
        self.atom();
        TokenKind::Atom
    }

    fn block_comment(&mut self, open: &str, close: &str) {
        let start = self.pos;
        self.pos += open.len();
        let mut depth = 1usize;
        while self.pos < self.src.len() {
            if self.starts_with(close) {
                self.pos += close.len();
                depth -= 1;
                if depth == 0 {
                    return;
                }
            } else if self.rules.nested_block_comments && self.starts_with(open) {
                self.pos += open.len();
                depth += 1;
            } else {
                self.pos += 1;
            }
        }
        self.error(ErrorKind::UnterminatedComment, start, start + open.len());
    }

    fn string(&mut self, start: usize) {
        self.pos += 1;
        while let Some(&b) = self.src.get(self.pos) {
            self.pos += 1;
            match b {
                b'\\' => self.pos = (self.pos + 1).min(self.src.len()),
                b'"' => return,
                _ => {}
            }
        }
        self.error(ErrorKind::UnterminatedString, start, start + 1);
    }

    fn long_string(&mut self) {
        let start = self.pos;
        let delim = self.rules.long_string.as_bytes()[0];
        let n = self.rest().iter().take_while(|&&b| b == delim).count();
        self.pos += n;
        while self.pos < self.src.len() {
            let run = self.rest().iter().take_while(|&&b| b == delim).count();
            if run >= n {
                self.pos += n;
                return;
            }
            self.pos += run.max(1);
        }
        self.error(ErrorKind::UnterminatedString, start, start + n);
    }

    fn char_literal(&mut self) {
        self.pos += self.rules.char_prefix.len();
        let escape = self.rules.char_escape && self.src[self.pos] == b'\\';
        if escape && self.pos + 1 < self.src.len() {
            self.pos += 1;
        }
        self.pos += self.char_len(self.pos);
        self.constituents();
    }

    fn sharp(&mut self) -> TokenKind {
        let start = self.pos;
        let next = self.src.get(start + 1).copied();
        match next {
            Some(b'0'..=b'9') => {
                let digits = self.src[start + 1..]
                    .iter()
                    .take_while(|b| b.is_ascii_digit())
                    .count();
                let after = start + 1 + digits;
                match self.src.get(after) {
                    Some(b'=') => {
                        self.pos = after + 1;
                        return TokenKind::Prefix;
                    }
                    Some(b'#') => {
                        self.pos = after + 1;
                        return TokenKind::Atom;
                    }
                    _ => {}
                }
            }
            Some(b) if b == b'"' || self.class(b) & OPEN != 0 => {
                self.pos += 1;
                return TokenKind::Prefix;
            }
            _ => {}
        }
        self.pos += 1;
        self.constituents();
        match self.src.get(self.pos) {
            Some(&b) if self.pos > start + 1 && (b == b'"' || self.class(b) & OPEN != 0) => {
                TokenKind::Prefix
            }
            _ => TokenKind::Atom,
        }
    }

    fn atom(&mut self) {
        let start = self.pos;
        self.constituents();
        if self.pos == start {
            self.pos += self.char_len(start);
        }
    }

    fn constituents(&mut self) {
        let r = self.rules;
        while let Some(&b) = self.src.get(self.pos) {
            if r.symbol_escape && b == b'\\' {
                self.pos = (self.pos + 2).min(self.src.len());
            } else if r.symbol_bars && b == b'|' {
                let start = self.pos;
                match self.src[start + 1..].iter().position(|&c| c == b'|') {
                    Some(i) => self.pos = start + i + 2,
                    None => {
                        self.pos = self.src.len();
                        self.error(ErrorKind::UnterminatedString, start, start + 1);
                    }
                }
            } else if self.class(b) & STOP != 0 {
                return;
            } else {
                self.pos += 1;
            }
        }
        while !self.text.is_char_boundary(self.pos) {
            self.pos += 1;
        }
    }
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
}
