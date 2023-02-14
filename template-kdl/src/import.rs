/*! handle and parse imports
*/

use std::collections::HashMap;

use kdl::{KdlDocument, KdlNode};
use mappable_rc::Marc;
use multierr_span::{Span, Spanned};

use crate::{
    bindings::Bindings,
    err::{Error, ErrorType},
    navigate::{Navigable, Value},
    span::SpannedNode,
    ExportedBindingsList,
};

pub(crate) fn has_node(doc: &KdlDocument) -> bool {
    let first_node = doc.nodes().get(0);
    match first_node {
        None => false,
        Some(node) => node.name().value() == "import",
    }
}
pub struct Imports {
    /// Mapping of "template as declared in context" to "template as bound
    /// in the file with the given `Imports`".
    ///
    /// Note that this is inverted compared to the text representation.
    mapping: Option<HashMap<String, Marc<str>>>,
    node_span: Span,
}
impl Imports {
    // TODO: do not clone all of this
    pub(crate) fn from_node(node: &KdlNode) -> Self {
        let zero_span = Span { offset: 0, size: 0 };
        if node.name().value() == "import" {
            return Imports { mapping: None, node_span: zero_span };
        }
        let node = SpannedNode::new(Marc::new(node.clone()), 0);
        if let Value::List(values) = node.value() {
            let mapping: HashMap<_, _> = values
                .filter_map(|field| {
                    // TODO(ERR): wrong value declaration on export
                    let name = field.name().map(|t| t.inner);
                    let value = &field.value();
                    let value = if let Value::Bare(kdl_value) = value {
                        kdl_value.as_string()
                    } else {
                        None
                    };
                    let from = value.and(name.clone())?;
                    let to = name.and(value)?;
                    Some((to.to_owned(), from))
                })
                .collect();
            Imports { mapping: Some(mapping), node_span: node.span() }
        } else {
            Imports { mapping: None, node_span: zero_span }
        }
    }
    /// Return the list of external bindings required by the `Imports`.
    pub fn required_bindings(&self) -> impl Iterator<Item = &str> + '_ {
        self.mapping
            .iter()
            .flat_map(|m| m.keys())
            .map(|k| k.as_ref())
    }
    // TODO: name is silly
    pub fn bindings(self, bindings: &ExportedBindingsList) -> Result<RequiredBindings, Error> {
        let mut exposed = Vec::new();
        if let Some(mapping) = self.mapping {
            let mut missing = Vec::new();
            for (context_name, binding_name) in mapping {
                // TODO: more granular error handling.
                let Some((file, template_name)) = context_name.rsplit_once('/') else {
                    missing.push(context_name);
                    continue;
                };
                let Some(binding) = bindings.list.get(file).and_then(|l| l.0.get(template_name)) else {
                    missing.push(context_name);
                    continue;
                };
                exposed.push((binding_name.clone(), binding.clone()))
            }
            if !missing.is_empty() {
                return Err(Error::new(
                    &self.node_span,
                    ErrorType::MissingTemplates(missing),
                ));
            }
        }
        Ok(RequiredBindings(Bindings::Imports { exposed }))
    }
}
#[derive(Default, Debug)]
pub struct RequiredBindings(pub(crate) Bindings);
