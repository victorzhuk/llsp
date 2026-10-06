use rustc_hash::{FxHashMap, FxHashSet};

use crate::dialect::{BindingShape, Cell, DefSpec, Dialect, ParamSearch, Params, SymbolKind};
use crate::syntax::{Delim, NodeId, NodeKind, Tree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub label: String,
    pub params: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Def {
    pub name: String,
    pub key: String,
    pub kind: SymbolKind,
    pub start: u32,
    pub end: u32,
    pub name_start: u32,
    pub name_end: u32,
    pub signatures: Vec<Signature>,
    pub doc: Option<String>,
    pub namespace: Option<String>,
    pub indent: Option<u32>,
    pub cell: Cell,
}

#[derive(Debug, Clone)]
pub struct Binder {
    pub name: String,
    pub key: String,
    pub start: u32,
    pub end: u32,
    pub visible_from: u32,
    pub scope_start: u32,
    pub scope_end: u32,
    pub cell: Cell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Local(u32),
    Global,
}

#[derive(Debug, Clone)]
pub struct Occurrence {
    pub key: String,
    /// Range of the base name, without qualifier.
    pub start: u32,
    pub end: u32,
    pub qualifier: Option<String>,
    pub target: Target,
    pub node: NodeId,
    pub cell: Cell,
}

#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub defs: Vec<Def>,
    pub binders: Vec<Binder>,
    pub occurrences: Vec<Occurrence>,
    pub namespace: Option<String>,
    /// Offsets where a namespace form switches the current namespace, in order.
    pub namespace_starts: Vec<(u32, String)>,
    pub aliases: FxHashMap<String, String>,
    /// Names imported with `:refer`, by normalized name, to their namespace.
    pub refers: FxHashMap<String, String>,
    /// Binder indices per scope, grouped by normalized name so `resolve` is a
    /// hash lookup per ancestor instead of a scan of every binder in scope.
    scopes: FxHashMap<NodeId, FxHashMap<String, Vec<u32>>>,
    binder_nodes: FxHashMap<NodeId, u32>,
}

impl Analysis {
    pub fn new(tree: &Tree, dialect: &Dialect) -> Self {
        let mut w = Walker {
            tree,
            d: dialect,
            a: Analysis::default(),
            ns: None,
            name_cells: FxHashMap::default(),
        };
        w.run();
        w.a
    }

    /// Occurrence whose atom covers `offset` (inclusive end).
    pub fn occurrence_at(&self, offset: u32) -> Option<&Occurrence> {
        let i = self.occurrences.partition_point(|o| o.end < offset);
        self.occurrences
            .get(i)
            .filter(|o| o.start <= offset && offset <= o.end)
    }

    /// Binders visible at `offset`, innermost first.
    pub fn visible_binders(&self, offset: u32) -> impl Iterator<Item = &Binder> {
        let mut v: Vec<&Binder> = self
            .binders
            .iter()
            .filter(|b| b.visible_from <= offset && offset <= b.scope_end && b.scope_start < offset)
            .collect();
        v.sort_by_key(|b| std::cmp::Reverse(b.visible_from));
        v.into_iter()
    }

    pub fn binder_index_at(&self, offset: u32) -> Option<u32> {
        self.binders
            .iter()
            .position(|b| b.start <= offset && offset <= b.end)
            .map(|i| i as u32)
    }

    pub fn def_at(&self, offset: u32) -> Option<&Def> {
        self.defs
            .iter()
            .find(|d| d.name_start <= offset && offset <= d.name_end)
    }

    pub fn resolve_qualifier<'a>(&'a self, q: &'a str) -> &'a str {
        self.aliases.get(q).map_or(q, String::as_str)
    }

    /// Namespace an occurrence names through its qualifier or a `:refer`.
    pub fn explicit_namespace(&self, d: &Dialect, o: &Occurrence) -> Option<String> {
        match &o.qualifier {
            Some(q) => Some(d.normalize(self.resolve_qualifier(q)).into_owned()),
            None => self.refers.get(&o.key).cloned(),
        }
    }

    pub fn namespace_at(&self, offset: u32) -> Option<&str> {
        namespace_at(&self.namespace_starts, offset)
    }
}

/// Current namespace at `offset` given the sorted namespace switch points.
pub fn namespace_at(starts: &[(u32, String)], offset: u32) -> Option<&str> {
    let i = starts.partition_point(|(at, _)| *at <= offset);
    i.checked_sub(1).map(|i| starts[i].1.as_str())
}

struct Walker<'a> {
    tree: &'a Tree,
    d: &'a Dialect,
    a: Analysis,
    ns: Option<String>,
    name_cells: FxHashMap<NodeId, Cell>,
}

impl Walker<'_> {
    fn run(&mut self) {
        let t = self.tree;
        let mut stack: Vec<NodeId> = t.children(Tree::ROOT).iter().rev().copied().collect();
        while let Some(id) = stack.pop() {
            match t.node(id).kind {
                NodeKind::DatumComment => continue,
                NodeKind::Atom => self.atom(id),
                NodeKind::List(Delim::Paren) => self.list(id),
                _ => {}
            }
            stack.extend(t.children(id).iter().rev());
        }
        self.a.occurrences.sort_by_key(|o| o.start);
    }

    fn key(&self, text: &str) -> String {
        self.d.normalize(text).into_owned()
    }

    fn is_symbol(&self, id: NodeId) -> bool {
        self.tree.atom(id).is_some_and(|s| symbol_like(self.d, s))
    }

    fn list(&mut self, id: NodeId) {
        let Some(head) = self.tree.head(id) else {
            return;
        };
        let head = self.key(head);
        if self.d.is_namespace_form(&head) {
            self.namespace_form(id);
        }
        let spec = self.d.def_spec(&head).cloned().or_else(|| {
            (self.d.binding_shape(&head).is_none() && self.d.has_def_prefix(&head)).then(|| {
                DefSpec {
                    kind: if head.contains("macro") {
                        SymbolKind::Macro
                    } else {
                        SymbolKind::Function
                    },
                    name: 1,
                    params: None,
                    doc: None,
                    cell: Cell::Value,
                }
            })
        });
        let explicit = self.d.def_spec(&head).is_some();
        if let Some(spec) = spec
            && self.definition(id, &spec, explicit)
        {
            return;
        }
        if let Some(shape) = self.d.binding_shape(&head) {
            self.binding(id, shape);
        }
    }

    fn namespace_form(&mut self, id: NodeId) {
        let t = self.tree;
        let Some(arg) = t.child(id, 1) else {
            return;
        };
        let arg = if t.node(arg).kind == NodeKind::Prefix {
            match t.children(arg).last() {
                Some(&c) => c,
                None => return,
            }
        } else {
            arg
        };
        let Some(name) = t.atom(arg) else {
            return;
        };
        let name = name.trim_start_matches("#:").trim_start_matches(':');
        let ns = self.key(name);
        if self.a.namespace.is_none() {
            self.a.namespace = Some(ns.clone());
        }
        self.a.namespace_starts.push((t.node(id).start, ns.clone()));
        self.ns = Some(ns);

        for n in t.preorder(id) {
            if t.node(n).kind != NodeKind::List(Delim::Bracket) {
                continue;
            }
            let kids = t.children(n);
            let Some(first) = kids.first().and_then(|&k| t.atom(k)) else {
                continue;
            };
            for (i, &k) in kids.iter().enumerate() {
                let next = kids.get(i + 1).copied();
                match t.atom(k) {
                    Some(":as" | ":as-alias") => {
                        if let Some(alias) = next.and_then(|k| t.atom(k)) {
                            self.a.aliases.insert(alias.to_owned(), first.to_owned());
                        }
                    }
                    Some(":refer") => {
                        let Some(names) =
                            next.filter(|&k| t.node(k).kind == NodeKind::List(Delim::Bracket))
                        else {
                            continue;
                        };
                        for name in t.children(names).iter().filter_map(|&c| t.atom(c)) {
                            self.a.refers.insert(self.key(name), self.key(first));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Returns false when the form has no usable name.
    fn definition(&mut self, id: NodeId, spec: &DefSpec, explicit: bool) -> bool {
        let t = self.tree;
        let Some(mut name_node) = t.child(id, spec.name) else {
            return false;
        };
        if t.node(name_node).kind == NodeKind::Prefix {
            match t.children(name_node) {
                [_, .., last] => name_node = *last,
                _ => return false,
            }
        }
        let mut implicit_params = None;
        let name = match t.node(name_node).kind {
            NodeKind::Atom if self.is_symbol(name_node) => t.node_text(name_node).to_owned(),
            NodeKind::List(Delim::Paren) => {
                let Some(head) = t.child(name_node, 0).filter(|&h| self.is_symbol(h)) else {
                    return false;
                };
                if self.key(t.node_text(head)) == "setf" {
                    collapse(t.node_text(name_node))
                } else {
                    if spec.params.is_none()
                        && matches!(spec.kind, SymbolKind::Function | SymbolKind::Macro)
                    {
                        implicit_params = Some(name_node);
                    }
                    name_node = head;
                    t.node_text(head).to_owned()
                }
            }
            _ => return false,
        };
        self.name_cells.insert(name_node, spec.cell);

        let mut signatures = Vec::new();
        let mut param_scopes = Vec::new();
        match spec.params {
            Some(Params::At(i)) => {
                if let Some(p) = t.child(id, i) {
                    signatures.push(signature(t, p, 0));
                    param_scopes.push((p, id, 0));
                }
            }
            Some(Params::Search(ParamSearch::List)) => {
                let after = t.children(id).iter().skip(spec.name + 1);
                if let Some(&p) = after
                    .clone()
                    .find(|&&c| t.node(c).kind == NodeKind::List(Delim::Paren))
                {
                    signatures.push(signature(t, p, 0));
                    param_scopes.push((p, id, 0));
                }
            }
            Some(Params::Search(ParamSearch::Vector)) => {
                let after: Vec<NodeId> =
                    t.children(id).iter().skip(spec.name + 1).copied().collect();
                if let Some(&p) = after
                    .iter()
                    .find(|&&c| t.node(c).kind == NodeKind::List(Delim::Bracket))
                {
                    signatures.push(signature(t, p, 0));
                    param_scopes.push((p, id, 0));
                } else {
                    for &arity in &after {
                        let Some(p) = t
                            .child(arity, 0)
                            .filter(|&p| t.node(p).kind == NodeKind::List(Delim::Bracket))
                        else {
                            continue;
                        };
                        if t.node(arity).kind == NodeKind::List(Delim::Paren) {
                            signatures.push(signature(t, p, 0));
                            param_scopes.push((p, arity, 0));
                        }
                    }
                }
            }
            None => {
                if let Some(list) = implicit_params {
                    let sig = signature(t, list, 1);
                    signatures.push(sig);
                    param_scopes.push((list, id, 1));
                }
            }
        }
        for (params, scope, skip) in param_scopes {
            let end = t.node(params).end;
            self.bind_params(params, skip, scope, end);
        }

        let kids = t.children(id);
        let doc = match spec.doc {
            Some(i) => t.child(id, i).filter(|&c| t.node(c).kind == NodeKind::Str),
            None => kids
                .iter()
                .enumerate()
                .skip(spec.name + 1)
                .find(|&(i, &c)| t.node(c).kind == NodeKind::Str && i + 1 < kids.len())
                .map(|(_, &c)| c),
        }
        .map(|c| unquote(t.node_text(c)));

        let mut kind = spec.kind;
        if explicit
            && kind == SymbolKind::Function
            && spec.params.is_none()
            && implicit_params.is_none()
            && !t
                .child(id, spec.name + 1)
                .and_then(|v| t.head(v))
                .and_then(|h| self.d.binding_shape(&self.key(h)))
                .is_some_and(|s| matches!(s, BindingShape::Lambda | BindingShape::Fn))
        {
            kind = SymbolKind::Variable;
        }
        let n = t.node(id);
        let nn = t.node(name_node);
        let (name_start, name_end) = if name.starts_with('(') {
            let orig = t.node(t.child(id, spec.name).unwrap_or(name_node));
            (orig.start, orig.end)
        } else {
            (nn.start, nn.end)
        };
        self.a.defs.push(Def {
            key: self.key(&name),
            name,
            kind,
            start: n.start,
            end: n.end,
            name_start,
            name_end,
            signatures,
            doc,
            namespace: self.ns.clone(),
            indent: self.indent_hint(id, spec.name),
            cell: spec.cell,
        });
        true
    }

    fn indent_hint(&self, id: NodeId, name_pos: usize) -> Option<u32> {
        let t = self.tree;
        for &c in t.children(id).iter().skip(name_pos) {
            let candidates: Vec<NodeId> = match t.node(c).kind {
                NodeKind::Prefix => t.children(c).to_vec(),
                _ => vec![c],
            };
            for c in candidates {
                let kids = t.children(c);
                match t.node(c).kind {
                    NodeKind::List(Delim::Paren) => {
                        let head = t.head(c);
                        for decl in self
                            .d
                            .indent_declarations
                            .iter()
                            .filter(|d| d.head.as_deref() == head)
                        {
                            let Some(name) = decl.name.as_deref() else {
                                continue;
                            };
                            for &decl_form in &kids[1..] {
                                if t.head(decl_form) == Some(name)
                                    && let Some(n) = t.child(decl_form, 1).and_then(|n| t.atom(n))
                                    && let Ok(n) = n.parse()
                                {
                                    return Some(n);
                                }
                            }
                        }
                    }
                    NodeKind::List(Delim::Brace) => {
                        for attr in self
                            .d
                            .indent_declarations
                            .iter()
                            .filter_map(|d| d.attribute.as_deref())
                        {
                            if let Some(i) = kids.iter().position(|&k| t.atom(k) == Some(attr))
                                && let Some(n) = kids.get(i + 1).and_then(|&n| t.atom(n))
                                && let Ok(n) = n.parse()
                            {
                                return Some(n);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    fn binding(&mut self, id: NodeId, shape: BindingShape) {
        let t = self.tree;
        let kids: Vec<NodeId> = t.children(id).to_vec();
        let end = |n: NodeId| t.node(n).end;
        match shape {
            BindingShape::Let => {
                let mut i = 1;
                if let Some(&name) = kids.get(1).filter(|&&n| self.is_symbol(n)) {
                    self.binder(name, id, end(name));
                    i = 2;
                }
                let Some(&bindings) = kids.get(i) else {
                    return;
                };
                if !matches!(t.node(bindings).kind, NodeKind::List(_)) {
                    return;
                }
                for &b in t.children(bindings) {
                    if self.is_symbol(b) {
                        self.binder(b, id, end(b));
                    } else if matches!(t.node(b).kind, NodeKind::List(_))
                        && let Some(p) = t.child(b, 0)
                    {
                        self.bind_pattern(p, id, end(b));
                    }
                }
            }
            BindingShape::LetVector => {
                if let Some(&v) = kids.get(1) {
                    self.let_vector(v, id);
                }
            }
            BindingShape::Lambda => {
                if let Some(&p) = kids.get(1) {
                    self.bind_params(p, 0, id, end(p));
                }
            }
            BindingShape::Fn => {
                let mut i = 1;
                if let Some(&name) = kids.get(1).filter(|&&n| self.is_symbol(n)) {
                    self.binder(name, id, end(name));
                    i = 2;
                }
                match kids.get(i) {
                    Some(&p) if t.node(p).kind == NodeKind::List(Delim::Bracket) => {
                        self.bind_params(p, 0, id, end(p));
                    }
                    _ => {
                        for &arity in &kids[i.min(kids.len())..] {
                            if t.node(arity).kind == NodeKind::List(Delim::Paren)
                                && let Some(p) = t.child(arity, 0)
                                && t.node(p).kind == NodeKind::List(Delim::Bracket)
                            {
                                self.bind_params(p, 0, arity, end(p));
                            }
                        }
                    }
                }
            }
            BindingShape::Bind => {
                if let Some(&p) = kids.get(1) {
                    let from = kids.get(2).map_or(end(p), |&v| end(v));
                    self.bind_pattern(p, id, from);
                }
            }
            BindingShape::Single => {
                if let Some(&spec) = kids.get(1)
                    && matches!(t.node(spec).kind, NodeKind::List(_))
                    && let Some(p) = t.child(spec, 0)
                {
                    self.bind_pattern(p, id, end(spec));
                }
            }
            BindingShape::Flet => {
                let Some(&defs) = kids.get(1) else {
                    return;
                };
                if !matches!(t.node(defs).kind, NodeKind::List(_)) {
                    return;
                }
                for &f in t.children(defs) {
                    if let Some(name) = t.child(f, 0).filter(|&n| self.is_symbol(n)) {
                        self.binder(name, id, end(defs));
                    }
                    if let Some(p) = t.child(f, 1) {
                        self.bind_params(p, 0, f, end(p));
                    }
                }
            }
            BindingShape::Iterator => {
                if let Some(&spec) = kids.get(1)
                    && let [patterns @ .., _] = t.children(spec)
                {
                    let mut seen = FxHashSet::default();
                    for &p in patterns {
                        self.collect_pattern(p, id, end(spec), &mut seen);
                    }
                }
            }
            BindingShape::Clauses => {
                for &clause in &kids[1.min(kids.len())..] {
                    if matches!(t.node(clause).kind, NodeKind::List(_))
                        && let Some(p) = t.child(clause, 0)
                    {
                        self.bind_params(p, 0, clause, end(p));
                    }
                }
            }
        }
    }

    fn let_vector(&mut self, v: NodeId, scope: NodeId) {
        let t = self.tree;
        if !matches!(t.node(v).kind, NodeKind::List(_)) {
            return;
        }
        let items: Vec<NodeId> = t.children(v).to_vec();
        for pair in items.chunks(2) {
            let (pat, init) = (pair[0], pair.get(1).copied());
            let from = t.node(init.unwrap_or(pat)).end;
            match t.atom(pat) {
                Some(":let") => {
                    if let Some(init) = init {
                        self.let_vector(init, scope);
                    }
                }
                Some(k) if self.d.is_keyword(k) => {}
                _ => self.bind_pattern(pat, scope, from),
            }
        }
    }

    fn bind_params(&mut self, params: NodeId, skip: usize, scope: NodeId, from: u32) {
        let t = self.tree;
        if t.node(params).kind == NodeKind::Atom {
            self.bind_pattern(params, scope, from);
            return;
        }
        let mut after_marker = false;
        let mut seen = FxHashSet::default();
        for &p in t.children(params).iter().skip(skip) {
            if let Some(a) = t.atom(p)
                && (a.starts_with('&') || self.d.is_pattern_ignored(&self.key(a)))
            {
                after_marker |= a.starts_with('&');
                continue;
            }
            if after_marker && t.node(p).kind == NodeKind::List(Delim::Paren) {
                if let Some(first) = t.child(p, 0) {
                    self.bind_one(first, scope, from, &mut seen);
                }
                if let Some(supplied) = t.child(p, 2) {
                    self.bind_one(supplied, scope, from, &mut seen);
                }
                continue;
            }
            self.collect_pattern(p, scope, from, &mut seen);
        }
    }

    fn bind_pattern(&mut self, pat: NodeId, scope: NodeId, from: u32) {
        let mut seen = FxHashSet::default();
        self.collect_pattern(pat, scope, from, &mut seen);
    }

    fn collect_pattern(
        &mut self,
        pat: NodeId,
        scope: NodeId,
        from: u32,
        seen: &mut FxHashSet<String>,
    ) {
        let t = self.tree;
        let mut stack = vec![pat];
        while let Some(n) = stack.pop() {
            match t.node(n).kind {
                NodeKind::Atom => self.bind_one(n, scope, from, seen),
                NodeKind::List(_) | NodeKind::Prefix => {
                    stack.extend(t.children(n).iter().rev());
                }
                _ => {}
            }
        }
    }

    fn bind_one(&mut self, n: NodeId, scope: NodeId, from: u32, seen: &mut FxHashSet<String>) {
        let Some(text) = self.tree.atom(n) else {
            return;
        };
        if !symbol_like(self.d, text) || text.starts_with('&') {
            return;
        }
        let key = self.key(text);
        if self.d.is_pattern_ignored(&key) || seen.contains(&key) {
            return;
        }
        seen.insert(key);
        self.binder(n, scope, from);
    }

    fn binder(&mut self, n: NodeId, scope: NodeId, visible_from: u32) {
        let t = self.tree;
        let text = t.node_text(n);
        let node = t.node(n);
        let s = t.node(scope);
        let key = self.key(text);
        let idx = self.a.binders.len() as u32;
        self.a.binders.push(Binder {
            name: text.to_owned(),
            key: key.clone(),
            start: node.start,
            end: node.end,
            visible_from,
            scope_start: s.start,
            scope_end: s.end,
            cell: Cell::Value,
        });
        self.a
            .scopes
            .entry(scope)
            .or_default()
            .entry(key)
            .or_default()
            .push(idx);
        self.a.binder_nodes.insert(n, idx);
    }

    fn atom(&mut self, id: NodeId) {
        let t = self.tree;
        let text = t.node_text(id);
        if !symbol_like(self.d, text) {
            return;
        }
        let (qualifier, base) = self.d.split_qualified(text);
        let node = t.node(id);
        let start = node.end - base.len() as u32;
        let key = self.key(base);
        let cell = if let Some(&b) = self.a.binder_nodes.get(&id) {
            self.a.binders[b as usize].cell
        } else if let Some(&c) = self.name_cells.get(&id) {
            c
        } else if self.function_cell(id) {
            Cell::Function
        } else {
            Cell::Value
        };
        let target = if let Some(&b) = self.a.binder_nodes.get(&id) {
            Target::Local(b)
        } else if qualifier.is_some() {
            Target::Global
        } else {
            self.resolve(id, &key, node.start, cell)
        };
        self.a.occurrences.push(Occurrence {
            key,
            start,
            end: node.end,
            qualifier: qualifier.map(str::to_owned),
            target,
            node: id,
            cell,
        });
    }

    /// Function cell for a call head, `#'` reference or `(function f)` argument,
    /// unless the list is a `cond` clause test or a reader vector's data.
    fn function_cell(&self, id: NodeId) -> bool {
        let t = self.tree;
        let Some(p) = t.parent(id) else {
            return false;
        };
        match t.node(p).kind {
            NodeKind::Prefix => under_function_ref(t, id),
            NodeKind::List(Delim::Paren) => {
                if t.child(p, 1) == Some(id) {
                    function_form_argument(t, self.d, id)
                } else {
                    t.child(p, 0) == Some(id)
                        && !(in_cond_clause(t, self.d, p) || in_reader_vector(t, p))
                }
            }
            _ => false,
        }
    }

    fn resolve(&self, id: NodeId, key: &str, offset: u32, cell: Cell) -> Target {
        for anc in self.tree.ancestors(id).skip(1) {
            let Some(ids) = self.a.scopes.get(&anc).and_then(|m| m.get(key)) else {
                continue;
            };
            let best = ids
                .iter()
                .filter(|&&b| {
                    let b = &self.a.binders[b as usize];
                    b.visible_from <= offset && self.d.cells_match(b.cell, cell)
                })
                .max_by_key(|&&b| self.a.binders[b as usize].visible_from);
            if let Some(&b) = best {
                return Target::Local(b);
            }
        }
        Target::Global
    }
}

/// True for atoms that name something: not numbers, keywords, constants or dispatch literals.
pub fn symbol_like(d: &Dialect, text: &str) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    if first == '#' || d.is_keyword(text) || looks_numeric(text) {
        return false;
    }
    !d.is_constant(&d.normalize(text))
}

/// True when `id` is the sole form under a `#'` prefix.
pub(crate) fn under_function_ref(t: &Tree, id: NodeId) -> bool {
    let Some(p) = t.parent(id) else {
        return false;
    };
    t.node(p).kind == NodeKind::Prefix && t.prefix_text(p) == "#'" && t.children(p) == [id]
}

/// True when `id` is the first argument of a `(function id)` form.
pub(crate) fn function_form_argument(t: &Tree, d: &Dialect, id: NodeId) -> bool {
    let Some(p) = t.parent(id) else {
        return false;
    };
    t.node(p).kind == NodeKind::List(Delim::Paren)
        && t.child(p, 1) == Some(id)
        && t.head(p).is_some_and(|h| d.normalize(h) == "function")
}

/// True when `id`'s parent list is a `(cond ...)` clause: its parent's head is
/// `cond` and `id`'s parent is not the head position itself.
pub(crate) fn in_cond_clause(t: &Tree, d: &Dialect, id: NodeId) -> bool {
    let Some(p) = t.parent(id) else {
        return false;
    };
    t.node(p).kind == NodeKind::List(Delim::Paren)
        && t.head(p).is_some_and(|h| d.normalize(h) == "cond")
        && t.child(p, 0) != Some(id)
}

/// True when `id`'s parent is a `#` prefix: a reader vector's data, `#(...)`.
pub(crate) fn in_reader_vector(t: &Tree, id: NodeId) -> bool {
    let Some(p) = t.parent(id) else {
        return false;
    };
    t.node(p).kind == NodeKind::Prefix && t.prefix_text(p) == "#"
}

fn looks_numeric(text: &str) -> bool {
    let body = text.strip_prefix(['+', '-']).unwrap_or(text);
    let body = body.strip_prefix('.').unwrap_or(body);
    body.starts_with(|c: char| c.is_ascii_digit()) && !text.ends_with(['+', '-'])
}

/// Parameters per signature are capped: signature labels and hover are computed
/// per request on the main loop, and pathological parameter lists would dominate.
const MAX_SIGNATURE_PARAMS: usize = 1024;

fn signature(t: &Tree, params: NodeId, skip: usize) -> Signature {
    let names: Vec<String> = t
        .children(params)
        .iter()
        .skip(skip)
        .take(MAX_SIGNATURE_PARAMS)
        .map(|&c| collapse(t.node_text(c)))
        .collect();
    let label = if skip == 0 {
        collapse(t.node_text(params))
    } else {
        format!("({})", names.join(" "))
    };
    Signature {
        label,
        params: names,
    }
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unquote(s: &str) -> String {
    let inner = if s.starts_with('`') {
        s.trim_matches('`')
    } else {
        s.strip_prefix('"')
            .map(|r| r.strip_suffix('"').unwrap_or(r))
            .unwrap_or(s)
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(c) => out.push(c),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests;
