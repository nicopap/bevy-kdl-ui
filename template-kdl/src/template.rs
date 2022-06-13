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

use kdl::{KdlDocument, KdlEntry, KdlIdentifier, KdlNode, KdlValue};
use mappable_rc::Marc;
use multierr_span::{Smarc, Span, Spanned};

use crate::bindings::Bindings;
use crate::err::{Error, ErrorType};
use crate::multi_err::{MultiError, MultiErrorTrait, MultiResult};
use crate::multi_try;
use crate::navigate::{Navigable, ThunkField_, Value};
use crate::span::{SpannedDocument, SpannedIdent, SpannedNode};

#[derive(Debug, Clone)]
pub(crate) enum TdefaultArg {
    None,
    Value(Smarc<KdlValue>),
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
    type Error = Error;
    fn try_from(node: SpannedNode) -> Result<Self, Self::Error> {
        let name = node.name();
        if name.value() == "expand" {
            // TODO error handling here (expand argument not string)
            let name = node.entries().next().unwrap().value();
            let name = name.as_string().unwrap().to_owned().into();
            let doc = node.children();
            let value = TdefaultArg::Expand(doc);
            Ok(Self { name, value })
        } else if let Some(children) = node.children() {
            let node_count = KdlDocument::nodes(&children).len();
            if node_count == 1 {
                // unwrap: we just checked node_count == 1
                let node = children.nodes().next().unwrap();
                Ok(Self {
                    name: name.value().to_owned().into(),
                    value: node.into(),
                })
            } else {
                Err(Error::new(&name, ErrorType::BadTemplateNodeParam))
            }
        } else {
            Err(Error::new(&name, ErrorType::BadTemplateNodeParam))
        }
    }
}
impl<'a> TryFrom<Smarc<KdlEntry>> for Tparameter {
    type Error = Error;
    fn try_from(entry: Smarc<KdlEntry>) -> Result<Self, Self::Error> {
        match (entry.name(), entry.value()) {
            (None, name) if name.is_string_value() => Ok(Self {
                name: name.as_string().unwrap().to_string().into(),
                value: TdefaultArg::None,
            }),
            (None, value) => Err(Error::new(
                &value,
                ErrorType::NonstringParam(KdlValue::clone(&value)),
            )),
            (Some(name), value) => Ok(Self {
                name: Marc::map(name.inner, |t| t.value()),
                value: TdefaultArg::Value(value),
            }),
        }
    }
}

/// Template arguments, the actual values with which a template is called.
#[derive(Default, Debug)]
pub(crate) struct Targuments {
    expand: HashMap<Marc<str>, Vec<NodeThunk>>,
    values: HashMap<Marc<str>, Smarc<KdlValue>>,
    nodes: HashMap<Marc<str>, NodeThunk>,
}
impl Targuments {
    fn expand(&self, key: &str) -> Option<Vec<NodeThunk>> {
        self.expand.get(key).cloned()
    }
    pub(crate) fn value(&self, key: &KdlValue) -> Option<&Smarc<KdlValue>> {
        let key = key.as_string()?;
        self.values.get(key)
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
    pub(crate) fn new(node: SpannedNode) -> MultiResult<Self, Error> {
        let mut errors = MultiError::default();
        let name = node.name();
        let no_child = || Error::new(&name, ErrorType::NoBody);
        let doc = multi_try!(errors, node.children().ok_or_else(no_child));
        let mut params: Vec<Tparameter> =
            errors.process_collect(node.entries().map(TryInto::try_into));
        let node_count = KdlDocument::nodes(&doc).len();
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
        let mut values = HashMap::<_, Smarc<_>>::default();
        let mut nodes = HashMap::default();
        let mut expand = HashMap::default();
        // default values
        for param in &self.params {
            match &param.value {
                TdefaultArg::Expand(Some(doc)) => {
                    let value = doc.nodes().map(|n| def_binds.thunk(n)).collect();
                    expand.insert(param.name.clone(), value);
                }
                TdefaultArg::Node(n) => {
                    nodes.insert(param.name.clone(), def_binds.thunk(n.clone()));
                }
                TdefaultArg::Value(v) => {
                    values.insert(param.name.clone(), v.clone());
                }
                TdefaultArg::Expand(None) => {}
                TdefaultArg::None => {}
            }
        }
        // get parameters from arguments
        if let Value::List(fields) = call.value() {
            for (i, field) in fields.enumerate() {
                let param = field
                    .name()
                    .and_then(|n| self.param_named(&*n))
                    .or_else(|| self.param_at(i));
                match (field.0, param) {
                    (ThunkField_::Entry(entry, ctx), Some(param)) => {
                        let value = entry.value();
                        let expanded = ctx.arguments.value(&*value).cloned();
                        let value = expanded.unwrap_or(value);
                        values.insert(param.name.clone(), value);
                    }
                    (ThunkField_::Entry(..), None) => {}
                    (ThunkField_::Node(argument), _) => match self.param_at(i) {
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
    pub(crate) arguments: Arc<Targuments>,
}

impl Context {
    pub(crate) fn new(bindings: Bindings) -> Self {
        Self { arguments: Default::default(), bindings }
    }
    // TODO: use a result here
    pub(crate) fn expand(&self, invocation: &NodeThunk) -> Vec<NodeThunk> {
        let invoke_name = invocation.name();
        // argument expension before binding expension, because that's what makes sense
        if let Value::List(mut list) = invocation.value() {
            if list.next().is_none() {
                if let Some(expanded) = self.arguments.node(invoke_name.value()).cloned() {
                    return vec![expanded];
                }
            }
        }
        if invoke_name.value() == "expand" {
            let expand_name = invocation.body.borrowed().entries().next().unwrap().value();
            let expand_name = expand_name.as_string().unwrap();
            return self.arguments.expand(expand_name).unwrap();
        }
        if let Some(thunk) = self.bindings.invoke(invocation) {
            return vec![thunk];
        }
        vec![]
    }
}

#[derive(Clone, Debug)]
pub struct NodeThunk {
    pub(crate) body: SpannedNode,
    pub(crate) context: Context,
}
impl Spanned for NodeThunk {
    fn span(&self) -> Span {
        self.body.span()
    }
}
impl NodeThunk {
    // true if has a single argument and no children.
    pub(crate) fn is_value(&self) -> bool {
        let inner = &self.body.inner;
        let single_entry = inner.entries().len() == 1;
        let first_name = inner.entries().first().map(|t| t.name());
        let no_name = first_name.map_or(false, |t| t.is_none());
        let no_children = inner.children().is_none();
        single_entry && no_name && no_children
    }
    pub(crate) fn new(body: SpannedNode, bindings: Bindings) -> Self {
        Self { body, context: Context::new(bindings) }
    }
    pub fn name(&self) -> SpannedIdent {
        self.body.name()
    }
    fn children(&self) -> impl Iterator<Item = NodeThunk> {
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
    /// Transform recursively this `NodeThunk` into a `KdlNode`.
    ///
    /// This forces full immediate evaluation. Other methods of `NodeThunk`,
    /// such as [`Self::children`], [`Self::entries`] and [`Self::fields`] are
    /// lazy and only substitute when necessary. You should prefer them to this
    /// method.
    ///
    /// This is useful for testing.
    pub fn evaluate(self) -> MultiResult<KdlNode, Error> {
        let mut errors = MultiError::default();
        let mut node = KdlNode::new(self.body.name().value());
        *node.entries_mut() = self
            .body
            .entries()
            .map(|e| {
                let value = e.value();
                let expanded = self.context.arguments.value(&*value).cloned();
                let value = KdlValue::clone(&expanded.unwrap_or(value));
                if let Some(name) = e.name() {
                    KdlEntry::new_prop(KdlIdentifier::clone(&name), value)
                } else {
                    KdlEntry::new(value)
                }
            })
            .collect();
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
}
impl fmt::Display for NodeThunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.body.inner)
    }
}
