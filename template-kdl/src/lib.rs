pub mod err;
pub mod multi_err;
pub mod span;
pub mod template;

use kdl::KdlDocument;

use err::Error;
use multi_err::{MultiError, MultiErrorTrait, MultiResult};
use span::{Span, Spanned, SpannedDocument};
use template::{Binding, Context, NodeThunk};

fn read_thunk(document: &KdlDocument) -> MultiResult<NodeThunk, Spanned<Error>> {
    let doc_len = document.len() as u32;
    let doc = SpannedDocument::new(document, 0);
    let mut errors = MultiError::default();
    let node_count = doc.node_count();
    if node_count == 0 {
        let err = Spanned(Span { offset: 0, size: doc_len }, err::Error::Empty);
        return errors.into_errors(err);
    }
    let mut all_nodes = doc.nodes().enumerate();
    let binding_nodes = all_nodes.by_ref().take(node_count - 1);
    let (bindings, binding_errors): (Vec<_>, Vec<_>) = binding_nodes
        .map(|(i, node)| Binding::new(i as u16, node))
        .unzip();
    errors.extend_errors(binding_errors.into_iter().flatten());
    let (_, last_node) = all_nodes.next().unwrap();
    let context = Context::new(bindings.into());
    errors.into_result(NodeThunk::new(last_node, context))
}

pub fn read_document(document: &KdlDocument) -> MultiResult<NodeThunk, Spanned<err::Error>> {
    read_thunk(document)
}
