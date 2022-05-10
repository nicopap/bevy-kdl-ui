//! Templates.
//!
//! At first, I implemented nodes resulting from template application as a
//! different kind of nodes.
//!
//! But it doesn't make sense! A "normal node" (ie: not in a template invocation)
//! is the same as a node in a template invocation, but where the argument list
//! is completely empty. So we basically have to replace the implementation of
//! `Spanned*` to account for the new substitution environment.
//!
//! I struggled several day until I reached that conclusion. Which in turn made
//! it real easy to finally implement templates.
//!
//! #### Variable scopping
//!
//! Now we introduced the concept of bindings and expensions, a few rules are
//! necessary:
//!
//! * When recursively expanding templates, variables should be properly managed
//! * When definining a `fn`, we should "capture" an immutable binding environment
//!
//! **binding set**: the set of variables that are declared and can be used in the
//! current scope. The difficulty comes from the fact that the binding set at the
//! call site of a template is different from the binding set at the declaration site.
//! And when expanding the template with its parameter, we must use the declaration
//! site binding set _for the body_, while using the call site binding set for the
//! nodes passed as argument at the _call site_.
//!
//! This means the scope and which binding set is active is tied to which node we are
//! looking at right now.
//!
//! At first I thought to add it to the `Context` struct in `visit.rs`, but a
//! problem I was getting is that I can't "push" and "pop" the binding set into the
//! context in a fool-proof way. I'll have to make sure everywhere I enter and leave
//! a scope to manually add and remove the binding set from the stack.
//!
//! The `NodeThunk` pairs a binding set with a node, this way, when walking
//! the node, the thunk will properly retrieve from the set the proper binding to
//! expand correctly the node elements.
// TODO: consider using a better hashmap implementation.
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use kdl::KdlValue;

use crate::err::{Error, Error::GenericUnsupported as TODO, Result};
use crate::span::{Span, SpannedEntry, SpannedNode};

#[derive(Debug)]
pub(crate) enum TdefaultArg<'s> {
    None,
    Value(Span, &'s KdlValue),
    Node(SpannedNode<'s>),
}
impl<'s> From<SpannedNode<'s>> for TdefaultArg<'s> {
    fn from(node: SpannedNode<'s>) -> Self {
        Self::Node(node)
    }
}
impl<'s> From<(Span, &'s KdlValue)> for TdefaultArg<'s> {
    fn from((span, value): (Span, &'s KdlValue)) -> Self {
        Self::Value(span, value)
    }
}
#[derive(Debug)]
pub(crate) struct Tparameter<'s> {
    name: &'s str,
    name_span: Span,
    value: TdefaultArg<'s>,
}
impl<'s> From<SpannedNode<'s>> for Tparameter<'s> {
    fn from(node: SpannedNode<'s>) -> Self {
        let (name_span, name) = node.name();
        Self { name, name_span, value: node.into() }
    }
}
impl<'s> TryFrom<SpannedEntry<'s>> for Tparameter<'s> {
    type Error = Error;
    fn try_from(entry: SpannedEntry<'s>) -> Result<Self> {
        if let Some((name_span, name)) = entry.name() {
            Ok(Self { name, name_span, value: entry.value().into() })
        } else {
            let (name_span, name) = entry.value();
            if let Some(name) = name.as_string() {
                Ok(Self { name, name_span, value: TdefaultArg::None })
            } else {
                Err(TODO("Bad function parameter".to_owned()))
            }
        }
    }
}
#[derive(Default)]
pub(crate) struct Targuments<'s> {
    values: HashMap<&'s str, (Span, &'s KdlValue)>,
    nodes: HashMap<&'s str, NodeThunk<'s>>,
}
impl<'s> Targuments<'s> {
    pub(crate) fn value(&self, key: &KdlValue) -> Option<(Span, &'s KdlValue)> {
        let key = key.as_string()?;
        self.values.get(key).cloned()
    }
    pub(crate) fn ident(&self, key: &str) -> Option<(Span, &'s str)> {
        let (s, v) = self.values.get(key)?;
        v.as_string().map(|v| (*s, v))
    }
    pub(crate) fn node(&self, key: &str) -> Option<&NodeThunk<'s>> {
        self.nodes.get(key)
    }
}
#[derive(Debug)]
pub(crate) struct Declaration<'s> {
    body: SpannedNode<'s>,
    params: Vec<Tparameter<'s>>,
}
impl<'s> Declaration<'s> {
    // TODO: define the DeclarError enum, and add it to the ConvError one.
    // TODO: should be able to return multiple errors
    pub(crate) fn new(node: SpannedNode<'s>) -> Result<Self> {
        let _ = node.name();
        let mut params = Vec::new();
        for entry in node.entries().1 {
            params.push(entry.try_into()?);
        }
        let no_child = || TODO("declaration node should have at least 1 child".to_owned());
        let doc = node.children().ok_or_else(no_child)?;
        let mut nodes_remaining = doc.node_count();
        let mut nodes = doc.nodes();
        while let Some(node) = nodes.next() {
            nodes_remaining -= 1;
            if nodes_remaining == 0 {
                return Ok(Self { body: node, params });
            }
            params.push(node.into());
        }
        Err(no_child())
    }
    /// Transform tparameters into targuments as specified at `call` site.
    pub(crate) fn arguments(
        &self,
        _call: NodeThunk<'s>,
        binding_index: usize,
        bindings: Rc<[Binding<'s>]>,
    ) -> Targuments<'s> {
        // TODO: clean this up, maybe use RwStruct
        // TODO FIXME: currently just transparently passes the default values
        let mut values = HashMap::default();
        let mut nodes = HashMap::default();
        for param in self.params.iter() {
            match param.value {
                TdefaultArg::Node(n) => {
                    let context = Context {
                        binding_index,
                        bindings: bindings.clone(),
                        arguments: Rc::new(Targuments::default()),
                    };
                    nodes.insert(param.name, NodeThunk::new(n, context));
                }
                TdefaultArg::Value(s, v) => {
                    values.insert(param.name, (s, v));
                }
                TdefaultArg::None => {}
            }
        }
        Targuments { values, nodes }
    }
}
/// A name binding. Later usages of this will cause an expension.
#[derive(Debug)]
pub(crate) struct Binding<'s> {
    name: &'s str,
    /// Index of highest binding.
    binding_index: usize,
    /// the declaration itself. None if it was malformed.
    declaration: Option<Declaration<'s>>,
}

impl<'s> Binding<'s> {
    pub(crate) fn new(
        binding_index: usize,
        name: &'s str,
        declaration: Option<Declaration<'s>>,
    ) -> Self {
        Self { name, declaration, binding_index }
    }
    pub(crate) fn expand(
        &self,
        invocation: NodeThunk<'s>,
        bindings: Rc<[Binding<'s>]>,
    ) -> Option<NodeThunk<'s>> {
        if self.name == invocation.name().1 {
            if let Some(declaration) = &self.declaration {
                let arguments = Rc::new(declaration.arguments(
                    invocation,
                    self.binding_index,
                    bindings.clone(),
                ));
                let context = Context {
                    arguments,
                    bindings,
                    binding_index: self.binding_index,
                };
                Some(NodeThunk { context, body: declaration.body })
            } else {
                // TODO: ideally: keep the initial invocation if error, so that it's
                // possible to diagnose errors even in it.
                None
            }
        } else {
            None
        }
    }
}

/// Context used to resolve the abstract nodes into actual nodes.
#[derive(Clone)]
pub(crate) struct Context<'s> {
    binding_index: usize,
    bindings: Rc<[Binding<'s>]>,
    arguments: Rc<Targuments<'s>>,
}

impl<'s> Context<'s> {
    pub(crate) fn new(bindings: Rc<[Binding<'s>]>) -> Self {
        let binding_index = bindings.len();
        Self {
            bindings,
            binding_index,
            arguments: Rc::new(Targuments::default()),
        }
    }
    pub(crate) fn expand(&self, invocation: NodeThunk<'s>) -> Option<NodeThunk<'s>> {
        self.bindings
            .get(0..self.binding_index)
            .unwrap()
            .iter()
            .rev()
            .find_map(|b| b.expand(invocation.clone(), self.bindings.clone()))
    }
}

pub struct EntryThunk<'s> {
    body: SpannedEntry<'s>,
    context: Context<'s>,
}
impl<'s> EntryThunk<'s> {
    pub(crate) fn new(body: SpannedEntry<'s>, context: Context<'s>) -> Self {
        Self { body, context }
    }

    // TODO: do not replace names
    pub fn name(&self) -> Option<(Span, &'s str)> {
        let (span, name) = self.body.name()?;
        Some(self.context.arguments.ident(name).unwrap_or((span, name)))
    }
    pub fn value(&self) -> (Span, &'s KdlValue) {
        let (s, v) = self.body.value();
        self.context.arguments.value(v).unwrap_or((s, v))
    }
}
#[derive(Clone)]
pub struct NodeThunk<'s> {
    body: SpannedNode<'s>,
    context: Context<'s>,
}
impl<'s> NodeThunk<'s> {
    pub(crate) fn new(body: SpannedNode<'s>, context: Context<'s>) -> Self {
        Self { body, context }
    }
    pub fn name(&self) -> (Span, &'s str) {
        let (span, name) = self.body.name();
        self.context.arguments.ident(name).unwrap_or((span, name))
    }
    pub fn entries(&self) -> (Span, impl Iterator<Item = EntryThunk<'s>>) {
        let (span, entries) = self.body.entries();
        let args = self.context.clone();
        // TODO: move Entry expension here
        let entries = entries.map(move |body| EntryThunk::new(body, args.clone()));
        (span, entries)
    }
    pub fn children(&self) -> impl Iterator<Item = NodeThunk<'s>> {
        let context = self.context.clone();
        // TODO(PERF): find something slightly more efficient than comparing every node
        // name every encountered with all bindings.
        let with_param_expanded = move |body: SpannedNode<'s>| {
            let body = NodeThunk::new(body, context.clone());
            context.expand(body.clone()).unwrap_or(body)
        };
        let doc = self.body.children();
        doc.into_iter()
            .flat_map(|d| d.nodes())
            .map(with_param_expanded)
    }
}
impl<'s> fmt::Display for NodeThunk<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.body)
    }
}
