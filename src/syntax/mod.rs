mod lexer;
mod parser;

use crate::dialect::Dialect;

pub use lexer::lex;

pub const MAX_ERRORS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Whitespace,
    LineComment,
    BlockComment,
    DatumComment,
    Open,
    Close,
    String,
    Char,
    Prefix,
    Atom,
}

impl TokenKind {
    pub fn is_trivia(self) -> bool {
        matches!(
            self,
            Self::Whitespace | Self::LineComment | Self::BlockComment
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delim {
    Paren,
    Bracket,
    Brace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Root,
    List(Delim),
    Atom,
    Str,
    Char,
    Prefix,
    DatumComment,
}

pub type NodeId = u32;

#[derive(Debug, Clone, Copy)]
pub struct Node {
    pub kind: NodeKind,
    pub start: u32,
    pub end: u32,
    pub parent: NodeId,
    /// First token of the node: the atom itself, the opener, or the prefix.
    pub token: u32,
    children: u32,
    child_count: u32,
    pub closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    UnclosedDelimiter,
    UnexpectedCloser,
    MismatchedDelimiter,
    UnterminatedString,
    UnterminatedComment,
    MissingForm,
    InvalidSyntax,
}

impl ErrorKind {
    pub fn message(self) -> &'static str {
        match self {
            Self::UnclosedDelimiter => "unclosed delimiter",
            Self::UnexpectedCloser => "unexpected closing delimiter",
            Self::MismatchedDelimiter => "mismatched delimiter",
            Self::UnterminatedString => "unterminated string",
            Self::UnterminatedComment => "unterminated block comment",
            Self::MissingForm => "prefix without a following form",
            Self::InvalidSyntax => "invalid syntax for this dialect",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::UnclosedDelimiter => "unclosed-delimiter",
            Self::UnexpectedCloser => "unexpected-closer",
            Self::MismatchedDelimiter => "mismatched-delimiter",
            Self::UnterminatedString => "unterminated-string",
            Self::UnterminatedComment => "unterminated-comment",
            Self::MissingForm => "missing-form",
            Self::InvalidSyntax => "invalid-syntax",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxError {
    pub kind: ErrorKind,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone)]
pub struct Tree {
    text: String,
    tokens: Vec<Token>,
    nodes: Vec<Node>,
    child_ids: Vec<NodeId>,
    errors: Vec<SyntaxError>,
}

impl Tree {
    pub const ROOT: NodeId = 0;

    pub fn parse(text: String, dialect: &Dialect) -> Tree {
        parser::parse(text, dialect)
    }

    /// Keeps `text` without reading it: one whitespace token, no forms.
    pub fn unparsed(text: String) -> Tree {
        let tokens = if text.is_empty() {
            Vec::new()
        } else {
            vec![Token {
                kind: TokenKind::Whitespace,
                start: 0,
                end: text.len() as u32,
            }]
        };
        let root = Node {
            kind: NodeKind::Root,
            start: 0,
            end: text.len() as u32,
            parent: Tree::ROOT,
            token: 0,
            children: 0,
            child_count: 0,
            closed: true,
        };
        Tree {
            text,
            tokens,
            nodes: vec![root],
            child_ids: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn errors(&self) -> &[SyntaxError] {
        &self.errors
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        let n = self.node(id);
        &self.child_ids[n.children as usize..(n.children + n.child_count) as usize]
    }

    pub fn child(&self, id: NodeId, i: usize) -> Option<NodeId> {
        self.children(id).get(i).copied()
    }

    pub fn token_text(&self, t: &Token) -> &str {
        &self.text[t.start as usize..t.end as usize]
    }

    pub fn node_text(&self, id: NodeId) -> &str {
        let n = self.node(id);
        &self.text[n.start as usize..n.end as usize]
    }

    /// Text of the prefix token for `Prefix` nodes (`'`, `#+`, `#hash`, ...).
    pub fn prefix_text(&self, id: NodeId) -> &str {
        self.token_text(&self.tokens[self.node(id).token as usize])
    }

    /// The node's text when it is an atom.
    pub fn atom(&self, id: NodeId) -> Option<&str> {
        (self.node(id).kind == NodeKind::Atom).then(|| self.node_text(id))
    }

    /// Head symbol of a parenthesized list.
    pub fn head(&self, id: NodeId) -> Option<&str> {
        if !matches!(self.node(id).kind, NodeKind::List(Delim::Paren)) {
            return None;
        }
        self.atom(self.child(id, 0)?)
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        (id != Self::ROOT).then(|| self.node(id).parent)
    }

    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        std::iter::successors(Some(id), |&n| self.parent(n))
    }

    /// Innermost node whose range contains `offset` (end-inclusive for leaves,
    /// so a cursor right after a symbol still finds it).
    pub fn node_at(&self, offset: u32) -> NodeId {
        let mut cur = Self::ROOT;
        loop {
            let kids = self.children(cur);
            let i = kids.partition_point(|&c| self.node(c).end < offset);
            let Some(&c) = kids.get(i) else {
                return cur;
            };
            let n = self.node(c);
            if n.start > offset
                || (n.end == offset && n.closed && matches!(n.kind, NodeKind::List(_)))
            {
                return cur;
            }
            cur = c;
        }
    }

    /// Innermost atom at or immediately before `offset`.
    pub fn atom_at(&self, offset: u32) -> Option<NodeId> {
        let id = self.node_at(offset);
        (self.node(id).kind == NodeKind::Atom).then_some(id)
    }

    /// True when `offset` sits inside a string or comment, including at the end of a
    /// line comment or an unterminated literal.
    pub fn in_literal_or_comment(&self, offset: u32) -> bool {
        let i = self.tokens.partition_point(|t| t.end < offset);
        let Some(t) = self.tokens.get(i) else {
            return false;
        };
        if t.start >= offset {
            return false;
        }
        match t.kind {
            TokenKind::LineComment => true,
            TokenKind::String | TokenKind::BlockComment => {
                offset < t.end || self.errors.iter().any(|e| e.start == t.start)
            }
            _ => false,
        }
    }

    /// Walks all nodes in document order without recursion.
    pub fn preorder(&self, from: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let mut stack = vec![from];
        std::iter::from_fn(move || {
            let id = stack.pop()?;
            stack.extend(self.children(id).iter().rev());
            Some(id)
        })
    }
}

#[cfg(test)]
mod tests;
