use kdl::KdlDocument;

use bevy_reflect::{TypeRegistry, Typed};
use template_kdl::{multi_err::MultiResult, navigate::ThunkField, Document, RequiredBindings};

use crate::{err::Error, newtype, ConvertResult, DynRefl};

pub fn read_navigable(
    field: ThunkField,
    expected: Option<&str>,
    registry: &TypeRegistry,
) -> MultiResult<DynRefl, Error> {
    newtype::make_named_dyn(registry, expected, field)
}
pub fn read_doc(
    doc: KdlDocument,
    expected: Option<&str>,
    registry: &TypeRegistry,
    required: RequiredBindings,
) -> ConvertResult {
    let doc_repr = doc.to_string();
    let result = template_kdl::read_document(doc, required).map_err(Error::from);
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
pub fn from_doc_untyped(
    doc: KdlDocument,
    bindings: RequiredBindings,
    registry: &TypeRegistry,
) -> ConvertResult {
    read_doc(doc, None, registry, bindings)
}
pub fn from_doc<T: Typed>(
    doc: KdlDocument,
    bindings: RequiredBindings,
    registry: &TypeRegistry,
) -> ConvertResult {
    let expected = Some(T::type_info().type_name());
    read_doc(doc, expected, registry, bindings)
}
