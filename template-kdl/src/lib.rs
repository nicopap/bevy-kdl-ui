mod bindings;
pub mod err;
mod field;
pub mod multi_err;
pub mod navigate;
pub mod span;
pub mod template;

use std::sync::Arc;

use kdl::KdlDocument;

use bindings::{Binding, Bindings as CrateBindings};
use err::{Error, ErrorType};
use mappable_rc::Marc;
use multi_err::{MultiError, MultiErrorTrait, MultiResult};
use navigate::{Navigable, Value};
use span::{SpannedDocument, SpannedNode};
use template::NodeThunk;

/// A parsed template KDL file.
pub enum Document {
    /// The file represents a node.
    Node(NodeThunk),
    /// The file exports bindings.
    Exports(Bindings),
}
#[derive(Debug, Default, Clone)]
pub struct Bindings {
    public_bindings: CrateBindings,
}
impl Bindings {
    // TODO(ERR): error handling when name not found in bindings
    fn from_export(bindings: CrateBindings, scope_name: String, exposed: SpannedNode) -> Self {
        if let Value::List(values) = exposed.value() {
            let binding_names: Vec<_> = values
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
                    Some((from, to.to_owned()))
                })
                .collect();
            let public_bindings = bindings.exports(scope_name, &binding_names);
            Self { public_bindings }
        } else {
            panic!()
        }
    }
}

pub fn read_document(
    document: KdlDocument,
    name: impl Into<String>,
    pre_bindings: Bindings,
) -> MultiResult<Document, Error> {
    let doc = SpannedDocument::new(Marc::new(document), 0);
    let mut errors = MultiError::default();
    let node_count = KdlDocument::nodes(&doc).len();
    if node_count == 0 {
        let err = Error::new(&doc, ErrorType::Empty);
        return errors.into_errors(err);
    }
    let mut all_nodes = doc.nodes();
    let binding_nodes = all_nodes.by_ref().take(node_count - 1);
    let bindings = binding_nodes.fold(pre_bindings.public_bindings, |bindings, body| {
        let (binding, errs) = Binding::new(body, bindings);
        errors.extend_errors(errs);
        CrateBindings::Local(Arc::new(binding))
    });
    let last_node = all_nodes.next().unwrap();
    if last_node.name().value() == "export" {
        let bindings = Bindings::from_export(bindings, name.into(), last_node);
        errors.into_result(Document::Exports(bindings))
    } else {
        let node = NodeThunk::new(last_node, bindings);
        errors.into_result(Document::Node(node))
    }
}

pub fn read_thunk(document: KdlDocument) -> MultiResult<NodeThunk, Error> {
    let err = Error::new(&(&document, 0), ErrorType::NotThunk);
    read_document(document, "read_thunk", Default::default()).and_then(|doc| match doc {
        Document::Exports(_) => MultiResult::Err(vec![err]),
        Document::Node(node) => MultiResult::Ok(node),
    })
}
