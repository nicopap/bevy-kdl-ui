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
use crate::span::{Span, SpannedDocument, SpannedEntry, SpannedNode};

#[derive(Debug)]
enum FdefaultArg<'s> {
    None,
    Value(Span, &'s KdlValue),
    Node(SpannedNode<'s>),
}
impl<'s> From<SpannedNode<'s>> for FdefaultArg<'s> {
    fn from(node: SpannedNode<'s>) -> Self {
        Self::Node(node)
    }
}
impl<'s> From<(Span, &'s KdlValue)> for FdefaultArg<'s> {
    fn from((span, value): (Span, &'s KdlValue)) -> Self {
        Self::Value(span, value)
    }
}
#[derive(Debug)]
struct Fparameter<'s> {
    name: &'s str,
    name_span: Span,
    value: FdefaultArg<'s>,
}
impl<'s> From<SpannedNode<'s>> for Fparameter<'s> {
    fn from(node: SpannedNode<'s>) -> Self {
        let (name_span, name) = node.name();
        Self { name, name_span, value: node.into() }
    }
}
impl<'s> TryFrom<SpannedEntry<'s>> for Fparameter<'s> {
    type Error = Error;
    fn try_from(entry: SpannedEntry<'s>) -> Result<Self> {
        if let Some((name_span, name)) = entry.name() {
            Ok(Self { name, name_span, value: entry.value().into() })
        } else {
            let (name_span, name) = entry.value();
            if let Some(name) = name.as_string() {
                Ok(Self { name, name_span, value: FdefaultArg::None })
            } else {
                Err(TODO("Bad function parameter".to_owned()))
            }
        }
    }
}
#[derive(Default)]
struct Farguments<'s, 'i> {
    values: HashMap<&'s str, (Span, &'s KdlValue)>,
    nodes: HashMap<&'s str, CallNode<'s, 'i>>,
}
impl<'s, 'i> Farguments<'s, 'i> {
    fn value(&self, key: &KdlValue) -> Option<(Span, &'s KdlValue)> {
        let key = key.as_string()?;
        self.values.get(key).cloned()
    }
    fn ident(&self, key: &str) -> Option<(Span, &'s str)> {
        let (s, v) = self.values.get(key)?;
        v.as_string().map(|v| (*s, v))
    }
    fn node(&self, key: &str) -> Option<&CallNode<'s, 'i>> {
        self.nodes.get(key)
    }
}
#[derive(Debug)]
pub struct Fdeclar<'s> {
    body: SpannedNode<'s>,
    params: Vec<Fparameter<'s>>,
}
impl<'s> Fdeclar<'s> {
    // TODO: define the DeclarError enum, and add it to the ConvError one.
    // TODO: should be able to return multiple errors
    pub fn new(node: SpannedNode<'s>) -> Result<Self> {
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
        Err(TODO(
            "declaration node should have at least 1 child".to_owned(),
        ))
    }
    fn arguments<'i>(
        &self,
        _invocation: CallNode<'s, 'i>,
        bindings: &'i [Binding<'s, 'i>],
    ) -> Farguments<'s, 'i> {
        // TODO: clean this up, maybe use RwStruct
        // TODO FIXME: currently just transparently passes the default values
        let mut values = HashMap::default();
        let mut nodes = HashMap::default();
        for param in self.params.iter() {
            match param.value {
                FdefaultArg::Node(n) => {
                    nodes.insert(param.name, CallNode::new(n, bindings.into()));
                }
                FdefaultArg::Value(s, v) => {
                    values.insert(param.name, (s, v));
                }
                FdefaultArg::None => {}
            }
        }
        Farguments { values, nodes }
    }
}
// TODO: a better API that hides Binding and Fdeclar
/// A name binding. Later usages of this will cause an expension.
#[derive(Debug)]
pub struct Binding<'s, 'i> {
    name: &'s str,
    /// Bindings at declaration site.
    bindings: &'i [Binding<'s, 'i>],
    /// the declaration itself. None if it was malformed.
    declaration: Option<Fdeclar<'s>>,
}

impl<'s, 'i> Binding<'s, 'i> {
    pub fn new(
        name: &'s str,
        bindings: &'i [Binding<'s, 'i>],
        declaration: Option<Fdeclar<'s>>,
    ) -> Self {
        Self { name, declaration, bindings }
    }
    fn expand(&self, invocation: CallNode<'s, 'i>) -> Option<CallNode<'s, 'i>> {
        if self.name == invocation.name().1 {
            if let Some(declaration) = &self.declaration {
                let arguments = Rc::new(declaration.arguments(invocation, self.bindings));
                let bindings = Bindings { arguments, bindings: self.bindings };
                Some(CallNode { bindings, body: declaration.body })
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
pub struct Bindings<'s, 'i> {
    bindings: &'i [Binding<'s, 'i>],
    arguments: Rc<Farguments<'s, 'i>>,
}

impl<'s, 'i> Bindings<'s, 'i> {
    fn expand(&self, invocation: CallNode<'s, 'i>) -> Option<CallNode<'s, 'i>> {
        self.bindings
            .iter()
            .rev()
            .find_map(|b| b.expand(invocation.clone()))
    }
}
impl<'s, 'i> From<&'i [Binding<'s, 'i>]> for Bindings<'s, 'i> {
    fn from(bindings: &'i [Binding<'s, 'i>]) -> Self {
        Self {
            bindings,
            arguments: Rc::new(Farguments::default()),
        }
    }
}

pub struct CallDocument<'s, 'i> {
    body: SpannedDocument<'s>,
    bindings: Bindings<'s, 'i>,
}
impl<'s, 'i> CallDocument<'s, 'i> {
    fn new(body: SpannedDocument<'s>, bindings: Bindings<'s, 'i>) -> Self {
        Self { body, bindings }
    }

    pub fn nodes(&self) -> impl Iterator<Item = CallNode<'s, 'i>> {
        let bindings = self.bindings.clone();
        // TODO(PERF): find something slightly more efficient than comparing every node
        // name every encountered with all bindings.
        let with_param_expanded = move |body: SpannedNode<'s>| {
            let body = CallNode::new(body, bindings.clone());
            bindings.expand(body.clone()).unwrap_or(body)
        };
        self.body.nodes().map(with_param_expanded)
    }
}
pub struct CallEntry<'s, 'i> {
    body: SpannedEntry<'s>,
    bindings: Bindings<'s, 'i>,
}
impl<'s, 'i> CallEntry<'s, 'i> {
    fn new(body: SpannedEntry<'s>, bindings: Bindings<'s, 'i>) -> Self {
        Self { body, bindings }
    }

    pub fn name(&self) -> Option<(Span, &'s str)> {
        let (span, name) = self.body.name()?;
        Some(self.bindings.arguments.ident(name).unwrap_or((span, name)))
    }
    pub fn value(&self) -> (Span, &'s KdlValue) {
        let (s, v) = self.body.value();
        self.bindings.arguments.value(v).unwrap_or((s, v))
    }
}
#[derive(Clone)]
pub struct CallNode<'s, 'i> {
    body: SpannedNode<'s>,
    bindings: Bindings<'s, 'i>,
}
impl<'s, 'i> CallNode<'s, 'i> {
    pub fn new(body: SpannedNode<'s>, bindings: Bindings<'s, 'i>) -> Self {
        Self { body, bindings }
    }
    pub fn name(&self) -> (Span, &'s str) {
        let (span, name) = self.body.name();
        self.bindings.arguments.ident(name).unwrap_or((span, name))
    }
    pub fn entries(&self) -> (Span, impl Iterator<Item = CallEntry<'s, 'i>>) {
        let (span, entries) = self.body.entries();
        let args = self.bindings.clone();
        let entries = entries.map(move |body| CallEntry::new(body, args.clone()));
        (span, entries)
    }
    pub fn children(&self) -> Option<CallDocument<'s, 'i>> {
        let doc = self.body.children()?;
        Some(CallDocument::new(doc, self.bindings.clone()))
    }
}
impl<'s, 'i> fmt::Display for CallNode<'s, 'i> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.body)
    }
}
