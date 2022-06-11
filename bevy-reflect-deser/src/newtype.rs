use std::mem;

use bevy_reflect::{
    DynamicStruct, DynamicTuple, DynamicTupleStruct, TypeInfo, TypeRegistration, TypeRegistry,
};
use template_kdl::{
    multi_err::MultiResult,
    span::Spanned,
    template::{Field, ValueExt},
};

use crate::{
    dyn_wrappers::{new_dynamic_anonstruct, new_pairmap, AnonTupleInfo, Infos},
    err::{ConvertError, ConvertError::GenericUnsupported as TODO},
    visit::KdlConcrete,
    DynRefl,
};

#[derive(Debug)]
pub(crate) struct ExpectedType<'r> {
    // The potential types a X can be declared as in KDL
    tys: Vec<&'r TypeInfo>,
}
fn into_dyn<'a>(
    value: &ValueExt,
    expected: Option<&'a TypeInfo>,
    reg: &'a TypeRegistry,
) -> MultiResult<DynRefl, Spanned<ConvertError>> {
    use TypeInfo::{List, Map, Struct, Tuple, TupleStruct, Value};
    use ValueExt::Node;
    match (value, expected) {
        (Node(node), None) => AnonTupleInfo.new_dynamic(node, reg),
        (Node(node), Some(Map(v))) if node.is_anon() => new_pairmap(v, node, reg),
        (Node(node), Some(Map(v))) => v.new_dynamic(node, reg),
        (Node(node), Some(List(v))) => v.new_dynamic(node, reg),
        (Node(node), Some(Tuple(v))) => v.new_dynamic(node, reg),
        (Node(node), Some(Value(v))) => v.new_dynamic(node, reg),
        (Node(node), Some(Struct(v))) if !node.is_anon() => v.new_dynamic(node, reg),
        (Node(node), Some(Struct(v))) => new_dynamic_anonstruct(v, node, reg),
        (Node(node), Some(TupleStruct(v))) => v.new_dynamic(node, reg),
        (ValueExt::Value(value), Some(expected)) => KdlConcrete::from(value.1.clone())
            .into_dyn(expected)
            .map_err(|e| Spanned(value.0, e))
            .into(),
        (value, info) => MultiResult::Err(vec![Spanned(
            value.span(),
            TODO(format!("cannot turn node into type: {value:?} \n {info:?}")),
        )]),
    }
}
impl<'r> ExpectedType<'r> {
    pub(crate) fn tuple() -> Self {
        Self { tys: vec![] }
    }
    // TODO(PERF): this is extremely inneficient for deeply nested newtypes that are
    // declared as the topmost type (ie: not using the shortcut syntax) since
    // for each level of nest, we visit all inner nests one more time.
    pub(crate) fn into_dyn(
        self,
        field: &dyn Field,
        reg: &'r TypeRegistry,
    ) -> MultiResult<DynRefl, Spanned<ConvertError>> {
        use MultiResult::Ok as MultiOk;
        if self.tys.is_empty() {
            return into_dyn(&field.field_value(), None, reg);
        }
        // build the whole type from the most inner type. The most inner type is the last
        // of the `tys` array. The goal is to build a `foo` which is the most outer type
        // of the newtype. We can only build the `foo` if we have the `bars` that are inner
        // type of `foo`. We can assume the size of each struct is 1 due to constructor
        // restriction.
        let mut tys = self.tys.into_iter().rev();
        // unwrap: only constructor has at least one element to tys
        let first = tys.next().unwrap();
        let mut inner = into_dyn(&field.field_value(), Some(first), reg);
        for ty in tys {
            match (&mut inner, ty) {
                (MultiOk(ref mut inner), TypeInfo::Struct(info)) => {
                    let field = info.field_at(0).unwrap().name();
                    let mut acc = DynamicStruct::default();
                    acc.set_name(info.type_name().to_owned());
                    let old_inner = mem::replace(inner, Box::new(()));
                    acc.insert_boxed(field, old_inner);
                    *inner = Box::new(acc);
                }
                (MultiOk(ref mut inner), TypeInfo::Tuple(info)) => {
                    let mut acc = DynamicTuple::default();
                    acc.set_name(info.type_name().to_owned());
                    let old_inner = mem::replace(inner, Box::new(()));
                    acc.insert_boxed(old_inner);
                    *inner = Box::new(acc);
                }
                (MultiOk(ref mut inner), TypeInfo::TupleStruct(info)) => {
                    let mut acc = DynamicTupleStruct::default();
                    acc.set_name(info.type_name().to_owned());
                    let old_inner = mem::replace(inner, Box::new(()));
                    acc.insert_boxed(old_inner);
                    *inner = Box::new(acc);
                }
                _ => {
                    inner = into_dyn(&field.field_value(), Some(ty), reg);
                }
            }
        }
        inner
    }

    pub(crate) fn new(expected: &'r TypeRegistration, reg: &'r TypeRegistry) -> Self {
        use TypeInfo::{Struct, Tuple, TupleStruct};
        let mut tys = Vec::with_capacity(1);
        let mut expected = expected.type_info();
        loop {
            tys.push(expected);
            match expected {
                Struct(info) if info.field_len() == 1 => {
                    // unwrap: We just checked the length is 1
                    let field = info.field_at(0).unwrap();
                    // TODO: unwrap
                    expected = reg.get_type_info(field.type_id()).unwrap();
                }
                Tuple(info) if info.field_len() == 1 => {
                    // unwrap: We just checked the length is 1
                    let field = info.field_at(0).unwrap();
                    // TODO: unwrap
                    expected = reg.get_type_info(field.type_id()).unwrap();
                }
                TupleStruct(info) if info.field_len() == 1 => {
                    // unwrap: We just checked the length is 1
                    let field = info.field_at(0).unwrap();
                    // TODO: unwrap
                    expected = reg.get_type_info(field.type_id()).unwrap();
                }
                _ => return Self { tys },
            }
        }
    }
}
