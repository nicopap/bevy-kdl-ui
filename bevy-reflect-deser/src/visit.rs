use kdl::KdlDocument;

use bevy_reflect::{TypeRegistry, Typed};
use template_kdl::multi_err::MultiResult;
use template_kdl::{navigate::ThunkField, Bindings, Document};

use crate::{err::Error, newtype, ConvertResult, DynRefl};

pub fn read_navigable(
    field: ThunkField,
    expected: Option<&str>,
    registry: &TypeRegistry,
) -> MultiResult<DynRefl, Error> {
    newtype::make_named_dyn(registry, expected, field)
}
// TODO: accept additional bindings
// TODO: function to extract necessary bindings
pub fn read_doc(
    doc: KdlDocument,
    doc_name: String,
    expected: Option<&str>,
    registry: &TypeRegistry,
) -> ConvertResult {
    let doc_repr = doc.to_string();
    let result =
        template_kdl::read_document(doc, doc_name, Bindings::default()).map_err(Error::from);
    match result.into_result() {
        Err(errs) => ConvertResult::errors(doc_repr, errs),
        Ok(Document::Exports(exports)) => ConvertResult::Exports(exports),
        Ok(Document::Node(node)) => {
            match read_navigable(ThunkField::node(node), expected, registry).into_result() {
                Ok(dyn_value) => ConvertResult::Deserialized(dyn_value),
                Err(errs) => ConvertResult::errors(doc_repr, errs),
            }
        }
    }
}
pub fn from_doc_untyped(doc: KdlDocument, registry: &TypeRegistry) -> ConvertResult {
    read_doc(doc, "".to_owned(), None, registry)
}
pub fn from_doc<T: Typed>(doc: KdlDocument, registry: &TypeRegistry) -> ConvertResult {
    let expected = Some(T::type_info().type_name());
    read_doc(doc, "".to_owned(), expected, registry)
}
