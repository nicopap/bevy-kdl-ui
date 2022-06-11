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
use std::sync::Arc;

use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use mappable_rc::Marc;

use crate::bindings::Bindings;
use crate::err::Error;
use crate::multi_err::{MultiError, MultiErrorTrait, MultiResult};
use crate::multi_try;
use crate::span::{Span, Spanned, SpannedDocument, SpannedEntry, SpannedIdent, SpannedNode};

#[derive(Debug, Clone)]
pub(crate) enum TdefaultArg {
    None,
    Value(Spanned<KdlValue>),
    Node(SpannedNode),
    Expand(Option<SpannedDocument>),
}
impl From<SpannedNode> for TdefaultArg {
    fn from(node: SpannedNode) -> Self {
        Self::Node(node)
    }
}
/// Template parameters.
///
/// The specification of a template, part of its definition.
#[derive(Debug, Clone)]
pub(crate) struct Tparameter {
    /// The name of the parameter, used in the body for substitution and at
    /// call site for named call.
    name: Marc<str>,
    /// Default value to give to parameter when call site doesn't specify one.
    value: TdefaultArg,
}
impl TryFrom<SpannedNode> for Tparameter {
    type Error = Spanned<Error>;
    fn try_from(node: SpannedNode) -> Result<Self, Self::Error> {
        let name = node.name();
        if name.value() == "expand" {
            // TODO error handling here (expand argument not string)
            let name = node.entries().next().unwrap().value();
            let name = name.1.as_string().unwrap().to_owned().into();
            let doc = node.children();
            let value = TdefaultArg::Expand(doc);
            Ok(Self { name, value })
        } else if let Some(children) = node.children() {
            let node_count = children.node_count();
            if node_count == 1 {
                // unwrap: we just checked node_count == 1
                let node = children.nodes().next().unwrap();
                Ok(Self {
                    name: name.value().to_owned().into(),
                    value: node.into(),
                })
            } else {
                Err(Spanned(name.span(), Error::BadTemplateNodeParam))
            }
        } else {
            Err(Spanned(name.span(), Error::BadTemplateNodeParam))
        }
    }
}
impl<'a> TryFrom<SpannedEntry<'a>> for Tparameter {
    type Error = Spanned<Error>;
    fn try_from(entry: SpannedEntry) -> Result<Self, Self::Error> {
        match (entry.name(), entry.value()) {
            (None, Spanned(_, KdlValue::String(name))) => Ok(Self {
                name: name.clone().into(),
                value: TdefaultArg::None,
            }),
            (None, value_span) => Err(value_span.map_cloned(Error::NonstringParam)),
            (Some(name), value) => Ok(Self {
                name: name.value().to_owned().into(),
                value: TdefaultArg::Value(value.cloned()),
            }),
        }
    }
}

/// Template arguments, the actual values with which a template is called.
#[derive(Default, Debug)]
pub(crate) struct Targuments {
    expand: HashMap<Marc<str>, Vec<NodeThunk>>,
    values: HashMap<Marc<str>, Spanned<KdlValue>>,
    nodes: HashMap<Marc<str>, NodeThunk>,
}
impl Targuments {
    fn expand(&self, key: &str) -> Option<Vec<NodeThunk>> {
        self.expand.get(key).cloned()
    }
    pub(crate) fn value(&self, key: &KdlValue) -> Option<Spanned<KdlValue>> {
        let key = key.as_string()?;
        self.values.get(key).cloned()
    }
    pub(crate) fn node(&self, key: &str) -> Option<&NodeThunk> {
        self.nodes.get(key)
    }
}
#[derive(Debug, Clone)]
pub(crate) struct Declaration {
    body: SpannedNode,
    params: Vec<Tparameter>,
}
impl Declaration {
    fn param_named(&self, name: &str) -> Option<&Tparameter> {
        self.params.iter().find(|p| p.name.as_ref() == name)
    }
    fn param_at(&self, index: usize) -> Option<&Tparameter> {
        self.params.get(index)
    }
    pub(crate) fn new(node: SpannedNode) -> MultiResult<Self, Spanned<Error>> {
        let mut errors = MultiError::default();
        let name_span = node.name().span();
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
    pub(crate) fn call(&self, call: &NodeThunk, def_binds: &Bindings) -> NodeThunk {
        let mut values = HashMap::default();
        let mut nodes = HashMap::default();
        let mut expand = HashMap::default();
        // default values
        for param in self.params.iter() {
            match &param.value {
                TdefaultArg::Expand(Some(doc)) => {
                    let value = doc.nodes().map(|n| n.with_binds(def_binds)).collect();
                    expand.insert(param.name.clone(), value);
                }
                TdefaultArg::Node(n) => {
                    nodes.insert(param.name.clone(), n.clone().with_binds(def_binds));
                }
                TdefaultArg::Value(v) => {
                    values.insert(param.name.clone(), v.clone());
                }
                TdefaultArg::Expand(None) => {}
                TdefaultArg::None => {}
            }
        }
        // get parameters from arguments
        for (i, field) in call.fields().enumerate() {
            let param = field
                .field_name()
                .and_then(|n| self.param_named(n.value()))
                .or_else(|| self.param_at(i));
            match (field.field_value(), param) {
                (ValueExt::Value(argument), Some(param)) => {
                    values.insert(param.name.clone(), argument.clone());
                }
                (ValueExt::Value(_), None) => {}
                (ValueExt::Node(argument), _) => match self.param_at(i) {
                    Some(Tparameter { name, value: TdefaultArg::Expand(_) }) => {
                        expand.insert(name.clone(), argument.children().collect());
                    }
                    Some(Tparameter { name, .. }) => {
                        nodes.insert(name.clone(), argument);
                    }
                    _ => {}
                },
            }
        }
        let arguments = Targuments { values, nodes, expand };
        let context = Context {
            arguments: Arc::new(arguments),
            bindings: def_binds.clone(),
        };
        NodeThunk { context, body: self.body.clone() }
    }
}
/// Context used to resolve the abstract nodes into actual nodes.
#[derive(Clone, Debug)]
pub(crate) struct Context {
    bindings: Bindings,
    arguments: Arc<Targuments>,
}
trait SpannedNodeThunkExt {
    fn with_binds(self, bindings: &Bindings) -> NodeThunk;
}
impl SpannedNodeThunkExt for SpannedNode {
    fn with_binds(self, bindings: &Bindings) -> NodeThunk {
        NodeThunk {
            body: self,
            context: Context::new(bindings.clone()),
        }
    }
}

impl Context {
    pub(crate) fn new(bindings: Bindings) -> Self {
        Self { arguments: Default::default(), bindings }
    }
    // TODO: use a result here
    pub(crate) fn expand(&self, invocation: &NodeThunk) -> Vec<NodeThunk> {
        let invoke_name = invocation.name().value();
        // argument expension before binding expension, because that's what makes sense
        if invocation.fields().next().is_none() {
            if let Some(expanded) = self.arguments.node(invoke_name).cloned() {
                return vec![expanded];
            }
        }
        if invoke_name == "expand" {
            let expand_name = invocation.entries().next().unwrap();
            let expand_name = expand_name.value();
            let expand_name = expand_name.1.as_string().unwrap();
            return self.arguments.expand(expand_name).unwrap();
        }
        if let Some(thunk) = self.bindings.invoke(invocation) {
            return vec![thunk];
        }
        vec![]
    }
}

pub struct EntryThunk<'s> {
    body: SpannedEntry<'s>,
    context: Context,
}
impl<'s> EntryThunk<'s> {
    pub(crate) fn new(body: SpannedEntry<'s>, context: Context) -> Self {
        Self { body, context }
    }
    pub fn name(&self) -> Option<SpannedIdent> {
        self.body.name()
    }
    pub fn value(&self) -> Spanned<KdlValue> {
        let Spanned(s, v) = self.body.value();
        self.context
            .arguments
            .value(v)
            .unwrap_or(Spanned(s, v.clone()))
    }
    pub fn evaluate(self) -> KdlEntry {
        let value = self.value().1;
        match self.body.entry.name() {
            Some(name) => KdlEntry::new_prop(name.clone(), value),
            None => KdlEntry::new(value),
        }
    }
}
#[derive(Debug)]
pub enum ValueExt {
    Node(NodeThunk),
    Value(Spanned<KdlValue>),
}
impl fmt::Display for ValueExt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node(node) => write!(f, "{node}"),
            Self::Value(Spanned(_, value)) => write!(f, "{value}"),
        }
    }
}
impl ValueExt {
    pub fn span(&self) -> Span {
        match self {
            Self::Node(node) => node.span(),
            Self::Value(value) => value.0,
        }
    }
}
// TODO: divide into AnonField and NamedField
// So that the Option check on name is discraded
pub trait Field {
    fn ty(&self) -> Option<SpannedIdent>;
    fn field_name(&self) -> Option<SpannedIdent>;
    fn field_value(&self) -> ValueExt;
    fn span(&self) -> Span;
}
impl<'s> Field for EntryThunk<'s> {
    fn field_name(&self) -> Option<SpannedIdent> {
        self.body.name().filter(|n| n.value() != "-")
    }
    fn span(&self) -> Span {
        self.body.span()
    }
    fn ty(&self) -> Option<SpannedIdent> {
        self.body.ty()
    }
    fn field_value(&self) -> ValueExt {
        ValueExt::Value(self.value())
    }
}
impl Field for NodeThunk {
    fn field_name(&self) -> Option<SpannedIdent> {
        let name = self.body.name();
        (name.value() != "-").then(|| name)
    }
    fn span(&self) -> Span {
        self.body.span()
    }
    fn ty(&self) -> Option<SpannedIdent> {
        self.body.ty()
    }
    fn field_value(&self) -> ValueExt {
        ValueExt::Node(self.clone())
    }
}

#[derive(Clone, Debug)]
pub struct NodeThunk {
    pub(crate) body: SpannedNode,
    pub(crate) context: Context,
}
impl NodeThunk {
    pub(crate) fn new(body: SpannedNode, bindings: Bindings) -> Self {
        Self { body, context: Context::new(bindings) }
    }
    pub fn name(&self) -> SpannedIdent {
        self.body.name()
    }
    pub fn entries(&self) -> impl Iterator<Item = EntryThunk> {
        let entries = self.body.entries();
        let args = self.context.clone();
        // TODO: move Entry expension here
        entries.map(move |body| EntryThunk::new(body, args.clone()))
    }
    pub fn children(&self) -> impl Iterator<Item = NodeThunk> {
        let context = self.context.clone();
        // TODO(PERF): find something slightly more efficient than comparing every node
        // name every encountered with all bindings.
        let with_param_expanded = move |body: SpannedNode| {
            let body = NodeThunk { body, context: context.clone() };
            let replacement = context.expand(&body);
            if replacement.is_empty() {
                vec![body]
            } else {
                replacement
            }
        };
        let doc = self.body.children();
        doc.into_iter()
            .flat_map(|d| d.nodes())
            .flat_map(with_param_expanded)
    }
    pub fn fields(&self) -> impl Iterator<Item = Box<dyn Field + '_>> {
        let entries = self.entries().map::<Box<dyn Field>, _>(|t| Box::new(t));
        let children = self.children().map::<Box<dyn Field>, _>(|t| Box::new(t));
        entries.chain(children)
    }
    /// Transform recursively this `NodeThunk` into a `KdlNode`.
    ///
    /// This forces full immediate evaluation. Other methods of `NodeThunk`,
    /// such as [`Self::children`], [`Self::entries`] and [`Self::fields`] are
    /// lazy and only substitute when necessary. You should prefer them to this
    /// method.
    ///
    /// This is useful for testing.
    pub fn evaluate(self) -> MultiResult<KdlNode, Spanned<Error>> {
        let mut errors = MultiError::default();
        let mut node = KdlNode::new(self.body.name().value());
        *node.entries_mut() = self.entries().map(|e| e.evaluate()).collect();
        let children: MultiResult<Vec<KdlNode>, _> =
            self.children().map(|n| n.evaluate()).collect();
        let children = multi_try!(errors, children);
        if !children.is_empty() {
            let mut document = KdlDocument::new();
            *document.nodes_mut() = children;
            node.set_children(document);
        }
        errors.into_result(node)
    }
    // TODO: actual error handling (eg: check there is not more than 1 etc.)
    pub fn first_argument(&self) -> Option<Spanned<KdlValue>> {
        let mut entries = self.entries();
        let first = entries.next()?;
        if first.name().is_some() || entries.next().is_some() {
            return None;
        }
        Some(first.value())
    }

    pub fn is_anon(&self) -> bool {
        self.fields()
            .next()
            .map_or(false, |e| e.field_name().is_none())
    }
}
impl fmt::Display for NodeThunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.body)
    }
}
