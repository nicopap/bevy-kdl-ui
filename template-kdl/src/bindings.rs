use std::sync::Arc;

use mappable_rc::Marc;

use crate::{
    err::Error,
    span::SpannedNode,
    template::{Context, Declaration, NodeThunk},
};

#[derive(Debug, Clone)]
pub(crate) enum Bindings {
    Local(Arc<Binding>),
    Exported(Vec<Arc<Binding>>),
    Terminal,
}
impl Default for Bindings {
    fn default() -> Self {
        Self::Terminal
    }
}
impl Bindings {
    pub(crate) fn invoke(&self, invocation: &NodeThunk) -> Option<NodeThunk> {
        self.visit()
            .find_map(|binding| binding.try_invoke(invocation))
    }
    fn visit(&self) -> BindingsIter {
        BindingsIter { inner: self, exported_idx: 0 }
    }
    // TODO(ERR): error handling when name not found in bindings
    // TODO(PERF): Use `Cow` here
    pub(crate) fn exports(self, scope_name: String, exposed: &[(Marc<str>, String)]) -> Self {
        let expose_name = |binding_name: &str| {
            exposed
                .iter()
                .find_map(|(from, to)| (binding_name == &**from).then_some(to))
        };
        let exposed = self.visit().filter_map(|binding| {
            expose_name(binding.name.as_ref()).map(|new_name| Binding {
                name: new_name.to_owned().into(),
                scope: Some(scope_name.clone()),
                ..Binding::clone(&**binding)
            })
        });
        Self::Exported(exposed.map(Arc::new).collect())
    }
    pub(crate) fn thunk(&self, body: SpannedNode) -> NodeThunk {
        NodeThunk { body, context: Context::new(self.clone()) }
    }
}

pub(crate) struct BindingsIter<'a> {
    inner: &'a Bindings,
    exported_idx: u32,
}
impl<'a> Iterator for BindingsIter<'a> {
    type Item = &'a Arc<Binding>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.inner {
            Bindings::Exported(v) => {
                let v = v.get(self.exported_idx as usize)?;
                self.exported_idx += 1;
                Some(v)
            }
            Bindings::Local(current) => {
                self.inner = &current.bindings;
                Some(current)
            }
            Bindings::Terminal => None,
        }
    }
}
/// A name binding. Later usages of this will cause an expension.
#[derive(Debug, Clone)]
pub(crate) struct Binding {
    pub(crate) name: Marc<str>,
    pub(crate) scope: Option<String>,
    // TODO(PERF): This is a linked list of bindings. Probably kills
    // performance due to low cache locality. discussed in
    // decision.md#binding-list-issue-again
    pub(crate) bindings: Bindings,
    /// the declaration itself. None if it was malformed.
    pub(crate) declaration: Option<Declaration>,
}

impl Binding {
    pub(crate) fn new(node: SpannedNode, bindings: Bindings) -> (Binding, Vec<Error>) {
        Declaration::new(node.clone()).unwrap_opt(|declaration| Self {
            name: node.name().value().to_owned().into(),
            declaration,
            bindings,
            scope: None,
        })
    }
    fn try_invoke(&self, invocation: &NodeThunk) -> Option<NodeThunk> {
        // TODO use self.scope here
        if self.name.as_ref() != invocation.name().value() {
            return None;
        }
        self.declaration
            .as_ref()
            .map(|d| d.call(invocation, &self.bindings))
    }
}
