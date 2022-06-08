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

use kdl::{KdlDocument, KdlEntry, KdlIdentifier, KdlNode, KdlValue};

use crate::err::Error;
use crate::multi_err::{MultiError, MultiErrorTrait, MultiResult};
use crate::multi_try;
use crate::span::{Span, Spanned, SpannedDocument, SpannedEntry, SpannedNode};

#[derive(Debug)]
pub(crate) enum TdefaultArg<'s> {
    None,
    Value(Spanned<&'s KdlValue>),
    Node(SpannedNode<'s>),
    Expand(Option<SpannedDocument<'s>>),
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
        let Spanned(name_span, name) = node.name();
        if name.value() == "expand" {
            let name = node.entries().next().unwrap().value();
            let name = name.1.as_string().unwrap();
            let doc = node.children();
            let value = TdefaultArg::Expand(doc);
            Ok(Self { name, value })
        } else if let Some(children) = node.children() {
            let node_count = children.node_count();
            if node_count == 1 {
                // unwrap: we just checked node_count == 1
                let node = children.nodes().next().unwrap();
                Ok(Self { name: name.value(), value: node.into() })
            } else {
                Err(Spanned(name_span, Error::BadTemplateNodeParam))
            }
        } else {
            Err(Spanned(name_span, Error::BadTemplateNodeParam))
        }
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
            (Some(Spanned(_, name)), value) => Ok(Self {
                name: name.value(),
                value: TdefaultArg::Value(value),
            }),
        }
    }
}
#[derive(Default, Debug)]
pub(crate) struct Targuments<'s> {
    expand: HashMap<&'s str, Vec<NodeThunk<'s>>>,
    values: HashMap<&'s str, Spanned<&'s KdlValue>>,
    nodes: HashMap<&'s str, NodeThunk<'s>>,
}
impl<'s> Targuments<'s> {
    fn expand(&self, key: &str) -> Option<Vec<NodeThunk<'s>>> {
        self.expand.get(key).cloned()
    }
    pub(crate) fn value(&self, key: &KdlValue) -> Option<Spanned<&'s KdlValue>> {
        let key = key.as_string()?;
        self.values.get(key).cloned()
    }
    // TODO: usages of this method are probably eroneous
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
    fn param_named(&self, name: &str) -> Option<&Tparameter<'s>> {
        self.params.iter().find(|p| p.name == name)
    }
    fn param_at(&self, index: usize) -> Option<&Tparameter<'s>> {
        self.params.get(index)
    }
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
        call: NodeThunk<'s>,
        binding_index: u16,
        bindings: Rc<[Binding<'s>]>,
    ) -> Targuments<'s> {
        let mut values = HashMap::default();
        let mut nodes = HashMap::default();
        let mut expand = HashMap::default();
        for param in self.params.iter() {
            match param.value {
                TdefaultArg::Expand(Some(doc)) => {
                    let value = doc
                        .nodes()
                        .map(|node| {
                            let context = Context {
                                binding_index,
                                bindings: bindings.clone(),
                                arguments: Rc::new(Targuments::default()),
                            };
                            NodeThunk::new(node, context)
                        })
                        .collect();
                    expand.insert(param.name, value);
                }
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
                TdefaultArg::Expand(None) => {}
                TdefaultArg::None => {}
            }
        }
        for (i, argument) in call.fields().enumerate() {
            let FieldThunk { name, value, .. } = argument;
            match value {
                ValueExt::Value(argument) => {
                    let param = if let Some(Spanned(_, name)) = name {
                        self.param_named(name.value())
                    } else {
                        self.param_at(i)
                    };
                    if let Some(param) = param {
                        values.insert(param.name, argument);
                    }
                }
                ValueExt::Node(argument) => match self.param_at(i) {
                    Some(Tparameter { name, value: TdefaultArg::Expand(_) }) => {
                        expand.insert(*name, argument.children().collect());
                    }
                    Some(Tparameter { name, .. }) => {
                        nodes.insert(*name, argument);
                    }
                    _ => {}
                },
            }
        }
        Targuments { values, nodes, expand }
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
            name: name.value(),
            declaration,
            binding_index: index,
        })
    }
    fn try_invoke(
        &self,
        invocation: NodeThunk<'s>,
        bindings: Rc<[Binding<'s>]>,
    ) -> Option<NodeThunk<'s>> {
        if self.name != invocation.name().1 {
            return None;
        }
        let declaration = self.declaration.as_ref()?;
        let arguments =
            Rc::new(declaration.arguments(invocation, self.binding_index, bindings.clone()));
        let context = Context {
            arguments,
            bindings,
            binding_index: self.binding_index,
        };
        Some(NodeThunk { context, body: declaration.body })
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
    // TODO: use a result here
    pub(crate) fn expand(&self, invocation: NodeThunk<'s>) -> Vec<NodeThunk<'s>> {
        let invoke_name = invocation.name().1;
        // argument expension before binding expension, because that's what makes sense
        if invocation.fields().next().is_none() {
            if let Some(expanded) = self.arguments.node(invoke_name).cloned() {
                return vec![expanded];
            }
        }
        if invoke_name == "expand" {
            let expand_name = invocation.entries().next().unwrap();
            let expand_name = expand_name.value().1.as_string().unwrap();
            return self.arguments.expand(expand_name).unwrap();
        }
        self.bindings[0..self.binding_index as usize]
            .iter()
            .rev()
            .find_map(|b| b.try_invoke(invocation.clone(), self.bindings.clone()))
            .into_iter()
            .collect()
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
        self.body.ty().map(|t| t.map(|t| t.value()))
    }
    pub fn span(&self) -> Span {
        self.body.span()
    }
    pub fn name(&self) -> Option<Spanned<&'s str>> {
        self.body.name().map(|t| t.map(|t| t.value()))
    }
    pub fn value(&self) -> Spanned<&'s KdlValue> {
        let Spanned(s, v) = self.body.value();
        self.context.arguments.value(v).unwrap_or(Spanned(s, v))
    }
    pub fn evaluate(self) -> KdlEntry {
        let value = self.value().1;
        match self.body.entry.name() {
            Some(name) => KdlEntry::new_prop(name.clone(), value.clone()),
            None => KdlEntry::new(value.clone()),
        }
    }
}
#[derive(Debug)]
pub enum ValueExt<'s> {
    Node(NodeThunk<'s>),
    Value(Spanned<&'s KdlValue>),
}
impl<'s> fmt::Display for ValueExt<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Node(node) => write!(f, "{node}"),
            Self::Value(Spanned(_, value)) => write!(f, "{value}"),
        }
    }
}
impl<'s> ValueExt<'s> {
    pub fn span(&self) -> Span {
        match self {
            Self::Node(node) => node.span(),
            Self::Value(value) => value.0,
        }
    }
}
pub struct FieldThunk<'s> {
    pub ty: Option<Spanned<&'s KdlIdentifier>>,
    pub name: Option<Spanned<&'s KdlIdentifier>>,
    pub value: ValueExt<'s>,
    pub span: Span,
}

impl<'s> FieldThunk<'s> {
    fn new(
        declared_ty: Option<Spanned<&'s KdlIdentifier>>,
        field_name: Option<Spanned<&'s KdlIdentifier>>,
        value: ValueExt<'s>,
        span: Span,
    ) -> Self {
        let field_name = field_name.filter(|name| name.1.value() != "-");
        Self { ty: declared_ty, name: field_name, value, span }
    }
    pub fn pair(&self) -> Option<(FieldThunk<'s>, FieldThunk<'s>)> {
        match &self.value {
            ValueExt::Value(_) => None,
            ValueExt::Node(n) => {
                let mut fields = n.fields();
                let n1 = fields.next()?;
                let n2 = fields.next()?;
                Some((n1, n2))
            }
        }
    }
}

impl<'s> From<EntryThunk<'s>> for FieldThunk<'s> {
    fn from(entry: EntryThunk<'s>) -> Self {
        let span = entry.span();
        let value = ValueExt::Value(entry.value());
        Self::new(entry.body.ty(), entry.body.name(), value, span)
    }
}
impl<'s> From<NodeThunk<'s>> for FieldThunk<'s> {
    fn from(node: NodeThunk<'s>) -> Self {
        let span = node.span();
        Self::new(
            node.body.ty(),
            Some(node.body.name()),
            ValueExt::Node(node),
            span,
        )
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
        self.body.ty().map(|t| t.map(|t| t.value()))
    }
    pub fn name(&self) -> Spanned<&'s str> {
        let Spanned(span, name) = self.body.name();
        self.context
            .arguments
            .ident(name.value())
            .unwrap_or(Spanned(span, name.value()))
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
            let replacement = context.expand(body.clone());
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
    pub fn fields(&self) -> impl Iterator<Item = FieldThunk<'s>> {
        let entries = self.entries().map(Into::into);
        let children = self.children().map(Into::into);
        entries.chain(children)
    }
    pub fn evaluate(self) -> MultiResult<KdlNode, Spanned<Error>> {
        let mut errors = MultiError::default();
        let children: MultiResult<Vec<KdlNode>, _> =
            self.children().map(|n| n.evaluate()).collect();
        let entries: Vec<KdlEntry> = self.entries().map(|e| e.evaluate()).collect();
        let mut node = KdlNode::new(self.body.name().1.clone());
        *node.entries_mut() = entries;
        let children = multi_try!(errors, children);
        if !children.is_empty() {
            let mut document = KdlDocument::new();
            *document.nodes_mut() = children;
            node.set_children(document);
        }
        errors.into_result(node)
    }
    // TODO: actual error handling (eg: check there is not more than 1 etc.)
    pub fn first_argument(&self) -> Option<Spanned<&'s KdlValue>> {
        let mut entries = self.entries();
        let first = entries.next();
        let second = entries.next();
        second
            .is_none()
            .then(|| first)
            .flatten()
            .and_then(|f| f.name().is_none().then(|| f.value()))
    }

    pub fn is_anon(&self) -> bool {
        self.fields().next().map_or(false, |e| e.name.is_none())
    }
}
impl<'s> fmt::Display for NodeThunk<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.body)
    }
}
