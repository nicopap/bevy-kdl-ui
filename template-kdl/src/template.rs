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
//!
//! ## `bindings` & `binding_index`
//!
//! We used to store the bind set as a reference to a slice. It is now an index +
//! a RC to a slice (which I now doubt is any better..)
//!
//! This let us take ownership of the slice and hopefully move toward sharable
//! definitions. It also enable us to share the slice between many nodes without
//! having recourse to `unsafe`, and thus we can get rid of `appendlist` which was
//! unsound.
// TODO: consider using a better hashmap implementation.
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use kdl::KdlValue;

use crate::err::Error;
use crate::multi_err::{MultiError, MultiErrorTrait, MultiResult};
use crate::multi_try;
use crate::span::{Span, Spanned, SpannedEntry, SpannedNode};

#[derive(Debug)]
pub(crate) enum TdefaultArg<'s> {
    None,
    Value(Spanned<&'s KdlValue>),
    Node(SpannedNode<'s>),
}
impl<'s> From<SpannedNode<'s>> for TdefaultArg<'s> {
    fn from(node: SpannedNode<'s>) -> Self {
        Self::Node(node)
    }
}
#[derive(Debug)]
pub(crate) struct Tparameter<'s> {
    name: &'s str,
    value: TdefaultArg<'s>,
}
impl<'s> TryFrom<SpannedNode<'s>> for Tparameter<'s> {
    type Error = Spanned<Error>;
    fn try_from(node: SpannedNode<'s>) -> Result<Self, Self::Error> {
        // TODO: proper parsing of node parameters
        let Spanned(_, name) = node.name();
        Ok(Self { name, value: node.into() })
    }
}
impl<'s> TryFrom<SpannedEntry<'s>> for Tparameter<'s> {
    type Error = Spanned<Error>;
    fn try_from(entry: SpannedEntry<'s>) -> Result<Self, Self::Error> {
        match (entry.name(), entry.value()) {
            (None, Spanned(_, KdlValue::String(name))) => {
                Ok(Self { name, value: TdefaultArg::None })
            }
            (None, value_span) => Err(value_span.map_cloned(Error::NonstringParam)),
            (Some(Spanned(_, name)), value) => Ok(Self { name, value: TdefaultArg::Value(value) }),
        }
    }
}
#[derive(Default, Debug)]
pub(crate) struct Targuments<'s> {
    values: HashMap<&'s str, Spanned<&'s KdlValue>>,
    nodes: HashMap<&'s str, NodeThunk<'s>>,
}
impl<'s> Targuments<'s> {
    pub(crate) fn value(&self, key: &KdlValue) -> Option<Spanned<&'s KdlValue>> {
        let key = key.as_string()?;
        self.values.get(key).cloned()
    }
    pub(crate) fn ident(&self, key: &str) -> Option<Spanned<&'s str>> {
        let Spanned(s, v) = self.values.get(key)?;
        v.as_string().map(|v| Spanned(*s, v))
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
    fn new(node: SpannedNode<'s>) -> MultiResult<Self, Spanned<Error>> {
        let mut errors = MultiError::default();
        let Spanned(name_span, _) = node.name();
        let no_child = || Spanned(name_span, Error::NoBody);
        let doc = multi_try!(errors, node.children().ok_or_else(no_child));
        let mut params: Vec<Tparameter> =
            errors.process_collect(node.entries().map(TryInto::try_into));
        let node_count = doc.node_count();
        if node_count == 0 {
            return errors.into_errors(no_child());
        }
        let mut all_nodes = doc.nodes();
        let param_nodes = all_nodes.by_ref().take(node_count - 1);
        params.extend::<Vec<_>>(errors.process_collect(param_nodes.map(TryFrom::try_from)));
        let body = all_nodes.next().unwrap();
        errors.into_result(Self { body, params })
    }
    /// Transform tparameters into targuments as specified at `call` site.
    fn arguments(
        &self,
        _call: NodeThunk<'s>,
        binding_index: u16,
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
                TdefaultArg::Value(v) => {
                    values.insert(param.name, v);
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
    binding_index: u16,
    /// the declaration itself. None if it was malformed.
    declaration: Option<Declaration<'s>>,
}

impl<'s> Binding<'s> {
    pub(crate) fn new(index: u16, node: SpannedNode<'s>) -> (Binding, Vec<Spanned<Error>>) {
        let Spanned(_, name) = node.name();
        Declaration::new(node).unwrap_opt(|declaration| Self {
            name,
            declaration,
            binding_index: index,
        })
    }
    fn expand(
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
#[derive(Clone, Debug)]
pub(crate) struct Context<'s> {
    binding_index: u16,
    bindings: Rc<[Binding<'s>]>,
    arguments: Rc<Targuments<'s>>,
}

impl<'s> Context<'s> {
    pub(crate) fn new(bindings: Rc<[Binding<'s>]>) -> Self {
        Self {
            arguments: Rc::new(Targuments::default()),
            binding_index: bindings.len() as u16,
            bindings,
        }
    }
    pub(crate) fn expand(&self, invocation: NodeThunk<'s>) -> Option<NodeThunk<'s>> {
        self.bindings[0..self.binding_index as usize]
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
    pub fn ty(&self) -> Option<Spanned<&'s str>> {
        self.body.ty()
    }
    pub fn span(&self) -> Span {
        self.body.span()
    }

    // TODO: do not replace names
    pub fn name(&self) -> Option<Spanned<&'s str>> {
        let Spanned(span, name) = self.body.name()?;
        Some(
            self.context
                .arguments
                .ident(name)
                .unwrap_or(Spanned(span, name)),
        )
    }
    pub fn value(&self) -> Spanned<&'s KdlValue> {
        let Spanned(s, v) = self.body.value();
        self.context.arguments.value(v).unwrap_or(Spanned(s, v))
    }
}
#[derive(Clone, Debug)]
pub struct NodeThunk<'s> {
    body: SpannedNode<'s>,
    context: Context<'s>,
}
impl<'s> NodeThunk<'s> {
    pub(crate) fn new(body: SpannedNode<'s>, context: Context<'s>) -> Self {
        Self { body, context }
    }
    pub fn span(&self) -> Span {
        self.body.span()
    }
    pub fn ty(&self) -> Option<Spanned<&'s str>> {
        self.body.ty()
    }
    pub fn name(&self) -> Spanned<&'s str> {
        let Spanned(span, name) = self.body.name();
        self.context
            .arguments
            .ident(name)
            .unwrap_or(Spanned(span, name))
    }
    pub fn entries(&self) -> impl Iterator<Item = EntryThunk<'s>> {
        let entries = self.body.entries();
        let args = self.context.clone();
        // TODO: move Entry expension here
        entries.map(move |body| EntryThunk::new(body, args.clone()))
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
