pub mod err;
pub mod span;
pub mod template;

use kdl::KdlDocument;
use span::SpannedDocument;
use template::{Binding, Context, Declaration, NodeThunk};

struct TemplateKdlDeserializer<'doc> {
    thunk: NodeThunk<'doc>,
    errors: Vec<err::Error>,
}
impl<'de> TemplateKdlDeserializer<'de> {
    fn new(document: &'de KdlDocument) -> err::Result<Self> {
        let doc = SpannedDocument::new(document, 0);
        let mut errors = Vec::new();
        let mut nodes_remaining = doc.node_count() as isize;
        let mut nodes = doc.nodes().enumerate();
        let mut bindings = Vec::new();
        let (_, last_node) = loop {
            nodes_remaining -= 1;
            if nodes_remaining <= 0 {
                break nodes.next().ok_or(err::Error::Empty)?;
            }
            let (i, node) = nodes.next().unwrap();
            let (_, name) = node.name();
            let declaration = match Declaration::new(node) {
                Ok(ok) => Some(ok),
                Err(err) => {
                    errors.push(err);
                    None
                }
            };
            bindings.push(Binding::new(i, name, declaration));
        };
        let context = Context::new(bindings.into());
        let thunk = NodeThunk::new(last_node, context);
        Ok(Self { errors, thunk })
    }
}

pub fn read_document(document: &KdlDocument) -> (Option<NodeThunk>, Vec<err::Error>) {
    match TemplateKdlDeserializer::new(document) {
        Ok(deser) => (Some(deser.thunk), deser.errors),
        Err(err) => (None, vec![err]),
    }
}
