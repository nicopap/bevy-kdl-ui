mod bindings;
pub mod err;
mod field;
mod import;
pub mod multi_err;
pub mod navigate;
pub mod span;
pub mod template;

pub use import::Imports;
pub use import::RequiredBindings;

use std::{collections::HashMap, sync::Arc};

use kdl::KdlDocument;

use bindings::{Binding, Bindings};
use err::{Error, ErrorType};
use mappable_rc::Marc;
use multi_err::{MultiError, MultiErrorTrait, MultiResult};
use navigate::{Navigable, Value};
use span::{SpannedDocument, SpannedNode};
use template::NodeThunk;

/// A parsed template KDL file.
#[derive(Clone, Debug)]
pub enum Document {
    /// The file represents a node.
    Node(NodeThunk),
    /// The file exports bindings.
    Exports(ExportedBindings),
}
#[derive(Debug, Default, Clone)]
pub struct ExportedBindingsList<'b> {
    pub list: HashMap<&'b str, ExportedBindings>,
}
#[derive(Debug, Default, Clone)]
pub struct ExportedBindings(bindings::Export);
impl ExportedBindings {
    // TODO(ERR): error handling when name not found in bindings
    fn from_export(bindings: Bindings, exposed: SpannedNode) -> Self {
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
            Self(bindings.exports(&binding_names))
        } else {
            panic!()
        }
    }
}

/// Returns imports required to read the file.
pub fn get_imports(document: &KdlDocument) -> Result<Imports, Error> {
    let first_node = document.nodes().get(0);
    match first_node {
        None => Err(Error::new(&(&document, 0), ErrorType::NoBody)),
        Some(node) => Ok(Imports::from_node(node)),
    }
}
pub fn read_document(
    document: KdlDocument,
    required: RequiredBindings,
) -> MultiResult<Document, Error> {
    let has_import = import::has_node(&document);
    let doc = SpannedDocument::new(Marc::new(document), 0);
    let mut errors = MultiError::default();
    let node_count = KdlDocument::nodes(&doc).len();
    if node_count == 0 {
        let err = Error::new(&doc, ErrorType::Empty);
        return errors.into_errors(err);
    }
    let mut all_nodes = doc.nodes();
    if has_import {
        all_nodes.next().unwrap();
    }
    let binding_nodes = all_nodes.by_ref().take(node_count - 1);
    let bindings = binding_nodes.fold(required.0, |bindings, body| {
        let (binding, errs) = Binding::new(body, bindings);
        errors.extend_errors(errs);
        Bindings::Local(Arc::new(binding))
    });
    let last_node = all_nodes.next().unwrap();
    if last_node.name().value() == "export" {
        let bindings = ExportedBindings::from_export(bindings, last_node);
        errors.into_result(Document::Exports(bindings))
    } else {
        let node = NodeThunk::new(last_node, bindings);
        errors.into_result(Document::Node(node))
    }
}

pub fn read_thunk(document: KdlDocument) -> MultiResult<NodeThunk, Error> {
    let err = Error::new(&(&document, 0), ErrorType::NotThunk);
    read_document(document, Default::default()).and_then(|doc| match doc {
        Document::Exports(_) => MultiResult::Err(vec![err]),
        Document::Node(node) => MultiResult::Ok(node),
    })
}
