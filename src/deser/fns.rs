//! `fn` nodes.
//!
//! A "function" node that binds a node name to an arbitrary transformation
//! into another node. Whenever a node with the bound name is found, it will
//! be transformed into the child node of the `fn` declaration. It is an
//! error for `fn` nodes to have not exactly one child node.
//!
//! `fn` node entries are:
//! * argument at position 1, `binding`: the name by which the function will be
//!   refered later.
//! * Any other entries: inputs to the function call to substitute into the child
//!   node definition. Parameters enables default values.
//! * All children but the last one also act like parameters, this let you define
//!   default values for node arguments.
//! ```kdl
//! fn "binding" "arg1" "arg2" param1=default param2=default {
//!   other_node {
//!     arg1 "foo";
//!     special_node arg2 param1;
//!     // etc.
//!   }
//! }
//! ```
//!
//! # `expand` node
//!
//! This node is only available in the context of a `fn` node. `expand foo` will expand
//! the children of the argument foo into the encompassing document. `foo`'s arguments are
//! discarded, only the children are kept.
// TODO: consider using a better hashmap implementation.
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use kdl::KdlValue;

use super::err::ConvResult;
use super::kdl_spans::{SpannedDocument, SpannedEntry, SpannedNode};
use super::span::Span;
use super::ConvertError;
use super::ConvertError::GenericUnsupported as TODO;

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
    type Error = ConvertError;
    fn try_from(entry: SpannedEntry<'s>) -> Result<Self, Self::Error> {
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
pub(super) struct Fdeclar<'s> {
    body: SpannedNode<'s>,
    params: Vec<Fparameter<'s>>,
}
impl<'s> Fdeclar<'s> {
    // TODO: define the DeclarError enum, and add it to the ConvError one.
    // TODO: should be able to return multiple errors
    pub(super) fn new(node: SpannedNode<'s>) -> ConvResult<Self> {
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
pub(super) struct Binding<'s, 'i> {
    name: &'s str,
    /// Bindings at declaration site.
    bindings: &'i [Binding<'s, 'i>],
    /// the declaration itself. None if it was malformed.
    declaration: Option<Fdeclar<'s>>,
}

impl<'s, 'i> Binding<'s, 'i> {
    pub(super) fn new(
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
pub(super) struct Bindings<'s, 'i> {
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

pub(super) struct CallDocument<'s, 'i> {
    body: SpannedDocument<'s>,
    bindings: Bindings<'s, 'i>,
}
impl<'s, 'i> CallDocument<'s, 'i> {
    fn new(body: SpannedDocument<'s>, bindings: Bindings<'s, 'i>) -> Self {
        Self { body, bindings }
    }

    pub(super) fn nodes(&self) -> impl Iterator<Item = CallNode<'s, 'i>> {
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
pub(super) struct CallEntry<'s, 'i> {
    body: SpannedEntry<'s>,
    bindings: Bindings<'s, 'i>,
}
impl<'s, 'i> CallEntry<'s, 'i> {
    fn new(body: SpannedEntry<'s>, bindings: Bindings<'s, 'i>) -> Self {
        Self { body, bindings }
    }

    pub(super) fn name(&self) -> Option<(Span, &'s str)> {
        let (span, name) = self.body.name()?;
        Some(self.bindings.arguments.ident(name).unwrap_or((span, name)))
    }
    pub(super) fn value(&self) -> (Span, &'s KdlValue) {
        let (s, v) = self.body.value();
        self.bindings.arguments.value(v).unwrap_or((s, v))
    }
}
#[derive(Clone)]
pub(super) struct CallNode<'s, 'i> {
    body: SpannedNode<'s>,
    bindings: Bindings<'s, 'i>,
}
impl<'s, 'i> CallNode<'s, 'i> {
    pub(super) fn new(body: SpannedNode<'s>, bindings: Bindings<'s, 'i>) -> Self {
        Self { body, bindings }
    }
    pub(super) fn name(&self) -> (Span, &'s str) {
        let (span, name) = self.body.name();
        self.bindings.arguments.ident(name).unwrap_or((span, name))
    }
    pub(super) fn entries(&self) -> (Span, impl Iterator<Item = CallEntry<'s, 'i>>) {
        let (span, entries) = self.body.entries();
        let args = self.bindings.clone();
        let entries = entries.map(move |body| CallEntry::new(body, args.clone()));
        (span, entries)
    }
    pub(super) fn children(&self) -> Option<CallDocument<'s, 'i>> {
        let doc = self.body.children()?;
        Some(CallDocument::new(doc, self.bindings.clone()))
    }
}
impl<'s, 'i> fmt::Display for CallNode<'s, 'i> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.body)
    }
}
