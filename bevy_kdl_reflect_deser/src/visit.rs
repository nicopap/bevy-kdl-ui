use kdl::KdlDocument;

use bevy_reflect::{TypeRegistry, Typed};
use template_kdl::multi_err::MultiResult;
use template_kdl::ExportedBindingsList;
use template_kdl::{navigate::ThunkField, Document};

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
    bindings: &ExportedBindingsList,
) -> ConvertResult {
    let doc_repr = doc.to_string();
    let imports = match template_kdl::get_imports(&doc) {
        Ok(imports) => imports,
        Err(errs) => return ConvertResult::errors(doc_repr, vec![errs.into()]),
    };
    let required = match imports.bindings(bindings) {
        Ok(required) => required,
        Err(errs) => return ConvertResult::errors(doc_repr, vec![errs.into()]),
    };
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
    bindings: &ExportedBindingsList,
    registry: &TypeRegistry,
) -> ConvertResult {
    read_doc(doc, None, registry, bindings)
}
pub fn from_doc<T: Typed>(
    doc: KdlDocument,
    bindings: &ExportedBindingsList,
    registry: &TypeRegistry,
) -> ConvertResult {
    let expected = Some(T::type_info().type_name());
    read_doc(doc, expected, registry, bindings)
}
