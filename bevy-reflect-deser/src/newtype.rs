use bevy_reflect::{
    DynamicStruct, DynamicTuple, DynamicTupleStruct, TypeInfo, TypeRegistration, TypeRegistry,
};

use template_kdl::{multi_err::MultiResult, span::Spanned};

use crate::err::ConvertError;
use crate::visit::ValueExt;
use crate::DynRefl;

pub(crate) struct ExpectedType<'r> {
    // The potential types a X can be declared as in KDL
    tys: Vec<&'r TypeInfo>,
}
impl<'r> ExpectedType<'r> {
    pub(crate) fn into_dyn(
        self,
        value: ValueExt,
        reg: &'r TypeRegistry,
    ) -> MultiResult<DynRefl, Spanned<ConvertError>> {
        // build the whole type from the most inner type. The most inner type is the last
        // of the `tys` array. The goal is to build a `foo` which is the most outer type
        // of the newtype. We can only build the `foo` if we have the `bars` that are inner
        // type of `foo`. We can assume the size of each struct is 1 due to constructor
        // restriction.
        let mut tys = self.tys.into_iter().rev();
        // unwrap: only constructor has at least one element to tys
        let first = tys.next().unwrap();
        let mut inner = match value.into_dyn(first, reg) {
            MultiResult::Ok(inner) => inner,
            any_else => return any_else,
        };
        for ty in tys {
            match ty {
                TypeInfo::Struct(info) => {
                    let field = info.field_at(0).unwrap().name();
                    let mut acc = DynamicStruct::default();
                    acc.set_name(info.id().type_name().to_owned());
                    acc.insert_boxed(field, inner);
                    inner = Box::new(acc);
                }
                TypeInfo::Tuple(info) => {
                    let mut acc = DynamicTuple::default();
                    acc.set_name(info.id().type_name().to_owned());
                    acc.insert_boxed(inner);
                    inner = Box::new(acc);
                }
                TypeInfo::TupleStruct(info) => {
                    let mut acc = DynamicTupleStruct::default();
                    acc.set_name(info.id().type_name().to_owned());
                    acc.insert_boxed(inner);
                    inner = Box::new(acc);
                }
                _ => {
                    unreachable!("By construction, tys only contains Tuple/Struct")
                }
            }
        }
        MultiResult::Ok(inner)
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
                    expected = reg.get_type_info(field.id().type_id()).unwrap();
                }
                Tuple(info) if info.field_len() == 1 => {
                    // unwrap: We just checked the length is 1
                    let field = info.field_at(0).unwrap();
                    // TODO: unwrap
                    expected = reg.get_type_info(field.id().type_id()).unwrap();
                }
                TupleStruct(info) if info.field_len() == 1 => {
                    // unwrap: We just checked the length is 1
                    let field = info.field_at(0).unwrap();
                    // TODO: unwrap
                    expected = reg.get_type_info(field.id().type_id()).unwrap();
                }
                _ => return Self { tys },
            }
        }
    }
}
