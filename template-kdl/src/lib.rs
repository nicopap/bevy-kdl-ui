mod bindings;
pub mod err;
pub mod multi_err;
pub mod span;
pub mod template;

use std::sync::Arc;

use kdl::KdlDocument;

use bindings::{Binding, Bindings as CrateBindings};
use err::Error;
use multi_err::{MultiError, MultiErrorTrait, MultiResult};
use span::{Span, Spanned, SpannedDocument, SpannedNode};
use template::NodeThunk;

pub enum Document {
    Node(NodeThunk),
    Exports(Bindings),
}
#[derive(Debug, Default, Clone)]
pub struct Bindings {
    public_bindings: CrateBindings,
}
impl Bindings {
    // TODO(ERR): error handling when name not found in bindings
    fn from_export(bindings: CrateBindings, scope_name: String, exposed: SpannedNode) -> Self {
        let binding_names: Vec<_> = exposed
            .fields()
            .filter_map(|(name, value)| {
                // TODO(ERR): wrong value declaration on export
                let value = value.and_then(|v| v.as_string());
                let from = value.and(name)?;
                let to = name.and(value)?;
                Some((from, to))
            })
            .collect();
        let public_bindings = bindings.exports(scope_name, &binding_names);
        Self { public_bindings }
    }
}

pub fn read_document(
    document: KdlDocument,
    name: impl Into<String>,
    pre_bindings: Bindings,
) -> MultiResult<Document, Spanned<Error>> {
    let doc_len = document.len() as u32;
    let doc = SpannedDocument::new(document);
    let mut errors = MultiError::default();
    let node_count = doc.node_count();
    if node_count == 0 {
        let err = Spanned(Span { offset: 0, size: doc_len }, err::Error::Empty);
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

pub fn read_thunk(document: KdlDocument) -> MultiResult<NodeThunk, Spanned<Error>> {
    let doc_len = document.len() as u32;
    read_document(document, "read_thunk", Default::default()).and_then(|doc| match doc {
        Document::Exports(_) => MultiResult::Err(vec![Spanned(
            Span { offset: 0, size: doc_len },
            Error::NotThunk,
        )]),
        Document::Node(node) => MultiResult::Ok(node),
    })
}
