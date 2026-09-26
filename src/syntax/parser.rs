use super::{
    Delim, ErrorKind, MAX_ERRORS, Node, NodeId, NodeKind, SyntaxError, Token, TokenKind, Tree, lex,
};
use crate::dialect::Dialect;

enum Frame {
    List {
        node: NodeId,
        close: u8,
        mark: usize,
    },
    Prefix {
        node: NodeId,
        want: u8,
        mark: usize,
    },
}

impl Frame {
    fn node(&self) -> NodeId {
        match self {
            Frame::List { node, .. } | Frame::Prefix { node, .. } => *node,
        }
    }

    fn mark(&self) -> usize {
        match self {
            Frame::List { mark, .. } | Frame::Prefix { mark, .. } => *mark,
        }
    }
}

struct Parser<'a> {
    text: &'a str,
    tokens: &'a [Token],
    nodes: Vec<Node>,
    child_ids: Vec<NodeId>,
    pending: Vec<NodeId>,
    stack: Vec<Frame>,
    errors: Vec<SyntaxError>,
}

pub fn parse(text: String, dialect: &Dialect) -> Tree {
    let (tokens, mut errors) = lex(&text, &dialect.reader);
    let mut p = Parser {
        text: &text,
        tokens: &tokens,
        nodes: Vec::with_capacity(tokens.len() / 2 + 1),
        child_ids: Vec::with_capacity(tokens.len() / 2),
        pending: Vec::with_capacity(256),
        stack: Vec::with_capacity(64),
        errors: std::mem::take(&mut errors),
    };
    p.run(dialect);
    let Parser {
        nodes,
        child_ids,
        mut errors,
        ..
    } = p;
    errors.sort_by_key(|e| e.start);
    errors.truncate(MAX_ERRORS);
    Tree {
        text,
        tokens,
        nodes,
        child_ids,
        errors,
    }
}

impl Parser<'_> {
    fn run(&mut self, dialect: &Dialect) {
        self.new_node(NodeKind::Root, 0, 0);
        let mut i = 0;
        while i < self.tokens.len() {
            let t = self.tokens[i];
            let idx = i as u32;
            i += 1;
            match t.kind {
                k if k.is_trivia() => {}
                TokenKind::Open => {
                    let open = self.text.as_bytes()[t.start as usize];
                    let delim = match open {
                        b'[' => Delim::Bracket,
                        b'{' => Delim::Brace,
                        _ => Delim::Paren,
                    };
                    let close = dialect.reader.closers[open as usize];
                    let node = self.new_node(NodeKind::List(delim), t.start, idx);
                    self.stack.push(Frame::List {
                        node,
                        close,
                        mark: self.pending.len(),
                    });
                }
                TokenKind::Close => {
                    if self.close(t) {
                        i -= 1;
                    }
                }
                TokenKind::Prefix | TokenKind::DatumComment => {
                    let (kind, want) = if t.kind == TokenKind::Prefix {
                        let text = &self.text[t.start as usize..t.end as usize];
                        (NodeKind::Prefix, prefix_arity(dialect, text))
                    } else {
                        (NodeKind::DatumComment, 1)
                    };
                    let node = self.new_node(kind, t.start, idx);
                    self.nodes[node as usize].end = t.end;
                    self.stack.push(Frame::Prefix {
                        node,
                        want,
                        mark: self.pending.len(),
                    });
                }
                TokenKind::String | TokenKind::Char | TokenKind::Atom => {
                    let kind = match t.kind {
                        TokenKind::String => NodeKind::Str,
                        TokenKind::Char => NodeKind::Char,
                        _ => NodeKind::Atom,
                    };
                    let node = self.new_node(kind, t.start, idx);
                    let n = &mut self.nodes[node as usize];
                    n.end = t.end;
                    n.closed = true;
                    self.complete(node);
                }
                _ => unreachable!("trivia handled above"),
            }
        }
        let eof = self.text.len() as u32;
        while let Some(frame) = self.stack.pop() {
            let node = frame.node();
            let n = self.nodes[node as usize];
            let (kind, end) = match frame {
                Frame::List { .. } => (ErrorKind::UnclosedDelimiter, n.start + 1),
                Frame::Prefix { .. } => (ErrorKind::MissingForm, self.tokens[n.token as usize].end),
            };
            self.error(kind, n.start, end);
            let end = match frame {
                Frame::List { .. } => eof,
                Frame::Prefix { .. } => self.last_end(frame.mark()).unwrap_or(n.end),
            };
            self.finish(node, frame.mark(), end, false);
            self.complete(node);
        }
        self.finish(Tree::ROOT, 0, eof, true);
    }

    /// Returns true when the closer must be re-processed by an outer frame.
    fn close(&mut self, t: Token) -> bool {
        let byte = self.text.as_bytes()[t.start as usize];
        match self.stack.last() {
            None => {
                self.error(ErrorKind::UnexpectedCloser, t.start, t.end);
                false
            }
            Some(Frame::Prefix { node, mark, .. }) => {
                let (node, mark) = (*node, *mark);
                self.stack.pop();
                let tok = self.tokens[self.nodes[node as usize].token as usize];
                self.error(ErrorKind::MissingForm, tok.start, tok.end);
                let end = self.last_end(mark).unwrap_or(tok.end);
                self.finish(node, mark, end, false);
                self.complete(node);
                true
            }
            Some(Frame::List { node, close, mark }) => {
                let (node, close, mark) = (*node, *close, *mark);
                if close == byte {
                    self.stack.pop();
                    self.finish(node, mark, t.end, true);
                    self.complete(node);
                    return false;
                }
                let outer_match = self
                    .stack
                    .iter()
                    .rev()
                    .skip(1)
                    .any(|f| matches!(f, Frame::List { close, .. } if *close == byte));
                if outer_match {
                    self.stack.pop();
                    let start = self.nodes[node as usize].start;
                    self.error(ErrorKind::UnclosedDelimiter, start, start + 1);
                    let end = self.last_end(mark).unwrap_or(start + 1);
                    self.finish(node, mark, end, false);
                    self.complete(node);
                    true
                } else {
                    self.error(ErrorKind::MismatchedDelimiter, t.start, t.end);
                    false
                }
            }
        }
    }

    fn last_end(&self, mark: usize) -> Option<u32> {
        self.pending[mark..]
            .last()
            .map(|&c| self.nodes[c as usize].end)
    }

    fn complete(&mut self, node: NodeId) {
        self.pending.push(node);
        while let Some(&Frame::Prefix { node, want, mark }) = self.stack.last() {
            if self.pending.len() - mark < want as usize {
                return;
            }
            self.stack.pop();
            let end = self.last_end(mark).unwrap_or(self.nodes[node as usize].end);
            self.finish(node, mark, end, true);
            self.pending.push(node);
        }
    }

    fn new_node(&mut self, kind: NodeKind, start: u32, token: u32) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            kind,
            start,
            end: start,
            parent: Tree::ROOT,
            token,
            children: 0,
            child_count: 0,
            closed: false,
        });
        id
    }

    fn finish(&mut self, node: NodeId, mark: usize, end: u32, closed: bool) {
        let first = self.child_ids.len() as u32;
        for &c in &self.pending[mark..] {
            self.nodes[c as usize].parent = node;
        }
        self.child_ids.extend(self.pending.drain(mark..));
        let n = &mut self.nodes[node as usize];
        n.children = first;
        n.child_count = self.child_ids.len() as u32 - first;
        n.end = end;
        n.closed = closed;
    }

    fn error(&mut self, kind: ErrorKind, start: u32, end: u32) {
        if self.errors.len() < MAX_ERRORS * 2 {
            self.errors.push(SyntaxError { kind, start, end });
        }
    }
}

fn prefix_arity(dialect: &Dialect, text: &str) -> u8 {
    dialect
        .reader
        .sorted_prefixes
        .iter()
        .find(|(p, _)| p == text)
        .map_or(1, |(_, n)| (*n).max(1))
}
