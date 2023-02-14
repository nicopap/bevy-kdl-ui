use std::{
    any::{self, TypeId},
    fmt, mem,
};

use bevy_reflect::{
    DynamicStruct, DynamicTuple, DynamicTupleStruct, TypeInfo, TypeRegistration, TypeRegistry,
};
use kdl::KdlValue;
use multierr_span::{Smarc, Span, Spanned};
use template_kdl::{
    multi_err::{MultiError, MultiErrorTrait, MultiResult},
    multi_try,
    navigate::{Navigable, ThunkField, Value as Nvalue},
};

use crate::{
    dyn_wrappers,
    err::{Error, ErrorType as ErrTy, ErrorType::GenericUnsupported as TODO, MResult},
    DynRefl,
};

type Field = ThunkField;
type Reg = TypeRegistry;

pub(crate) fn make_dyn(reg: &Reg, expected: Option<&str>, field: Field) -> MResult<DynRefl> {
    let ty = field.ty();
    let ty_span = ty.as_ref().map_or_else(|| field.span(), |ty| ty.span());
    make_declared_dyn(reg, ty.as_deref(), expected, ty_span, field)
}
pub(crate) fn make_named_dyn(reg: &Reg, expected: Option<&str>, field: Field) -> MResult<DynRefl> {
    let ty = field.ty().or(field.name());
    let ty_span = ty.as_ref().map_or_else(|| field.span(), |ty| ty.span());
    make_declared_dyn(reg, ty.as_deref(), expected, ty_span, field)
}
fn make_declared_dyn(
    reg: &Reg,
    declared: Option<&str>,
    expected: Option<&str>,
    ty_span: Span,
    field: Field,
) -> MResult<DynRefl> {
    let mut errs = MultiError::default();
    let expected = multi_try!(errs, ExpectedType::new(reg, declared, expected, ty_span));
    expected.make_dyn(field).combine(errs)
}

struct ExpectedType<'r> {
    // The potential types a X can be declared as in KDL
    tys: Vec<&'r TypeInfo>,
    reg: &'r TypeRegistry,
}
impl<'r> ExpectedType<'r> {
    // TODO(PERF): this is extremely inneficient for deeply nested newtypes that are
    // declared as the topmost type (ie: not using the shortcut syntax) since
    // for each level of nest, we visit all inner nests one more time.
    fn make_dyn(self, field: Field) -> MResult<DynRefl> {
        use MultiResult::Ok as MultiOk;
        use Nvalue::{Bare, List as Vlist};

        let into_dyn = |expected| match (field.value(), expected) {
            (Vlist(_), info) => dyn_wrappers::from_expected(info, &field, self.reg),
            (Bare(value), Some(expected)) => KdlConcrete::from(value).into_dyn(expected).into(),
            (_, info) => {
                let msg = format!("cannot turn field into type: {field:?} \n {info:?}");
                MResult::Err(vec![TODO(msg).spanned(&field)])
            }
        };
        if self.tys.is_empty() {
            return into_dyn(None);
        }
        // build the whole type from the most inner type. The most inner type is the last
        // of the `tys` array. The goal is to build a `foo` which is the most outer type
        // of the newtype. We can only build the `foo` if we have the `bars` that are inner
        // type of `foo`. We can assume the size of each struct is 1 due to constructor
        // restriction.
        let mut tys = self.tys.into_iter().rev();
        // unwrap: only constructor has at least one element to tys
        let first = tys.next().unwrap();
        let mut inner = into_dyn(Some(first));
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
                    inner = into_dyn(Some(ty));
                }
            }
        }
        inner
    }

    fn registered(expected: &'r TypeRegistration, reg: &'r TypeRegistry) -> Self {
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
                _ => return Self { tys, reg },
            }
        }
    }

    // Possible failure states:
    // * declared is not registered
    // * expected is not registered
    // * declared is Some and does not match expected
    // * Any combination of the above
    // * Fatal: only if expected is not registered and (either declared is None or not registered)
    fn new(
        reg: &'r TypeRegistry,
        declared: Option<&str>,
        expected: Option<&str>,
        span: Span,
    ) -> MResult<Self> {
        let get_named = |name| {
            reg.get_with_name(name)
                .or_else(|| reg.get_with_short_name(name))
                .ok_or(ErrTy::NoSuchType(name.to_owned()).spanned(&span))
        };
        let mut errs = MultiError::default();
        let expected = expected.and_then(|e| errs.optionally(get_named(e)));
        match (declared, expected) {
            (Some("Tuple"), Some(expected)) => {
                return errs.into_result(Self::registered(expected, reg))
            }
            (Some("Tuple"), None) => {
                return errs.into_result(Self { tys: vec![], reg });
            }
            _ => {}
        }
        let declared = declared.and_then(|e| errs.optionally(get_named(e)));
        match (declared, expected) {
            // Both declared and expected are registered, but they are not equal
            // We chose `declared` since that's what is in the file, so we expect that
            // the rest of the file uses the declaredly stated type.
            (Some(declared), Some(expected)) if declared.type_id() != expected.type_id() => {
                let expected = expected.type_name();
                let actual = declared.type_name().to_owned();
                errs.add_error(ErrTy::TypeMismatch { expected, actual }.spanned(&span));
                errs.into_result(Self::registered(declared, reg))
            }
            // Either declared was not provided, or it was not registered (in which case
            // the error is already in `errors`) or it was provided, registered and matched
            // expected. And expected is registered
            (_, Some(expected)) => errs.into_result(Self::registered(expected, reg)),
            // Either declared was not provided, or it was not registered (in which case
            // the error is already in `errors`) and expected is not registered
            // NOTE: This is the only Fatal error preventing any validation of what's inside.
            (None, None) => errs.into_errors(ErrTy::UntypedTupleField.spanned(&span)),
            // declared type exists, but is not equal to expected one, and the
            // expected one is not registered. This is an error, but we continue,
            // hoping to be useful
            (Some(declared), None) => errs.into_result(Self::registered(declared, reg)),
        }
    }
}
/// A proxy for [`KdlValue`] that doesn't care about the format of declaration.
enum KdlType {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Null,
}
struct KdlConcrete {
    ty: KdlType,
    span: Span,
}
impl From<Smarc<KdlValue>> for KdlConcrete {
    fn from(value: Smarc<KdlValue>) -> Self {
        use KdlValue::{
            Base10, Base10Float, Base16, Base2, Base8, Bool, Null, RawString, String as VString,
        };
        let span = value.span();
        let ty = match &*value {
            Base10(i) | Base2(i) | Base16(i) | Base8(i) => KdlType::Int(*i),
            Base10Float(f) => KdlType::Float(*f),
            VString(s) | RawString(s) => KdlType::Str(s.clone()),
            Bool(b) => KdlType::Bool(*b),
            Null => KdlType::Null,
        };
        KdlConcrete { ty, span }
    }
}
impl KdlConcrete {
    fn into_dyn(self, expected: &TypeInfo) -> Result<DynRefl, Error> {
        self.ty
            .into_dyn(expected)
            .map_err(|e| Error::new(&self.span, e))
    }
}
impl fmt::Display for KdlType {
    fn fmt(&self, fm: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            KdlType::Int(i) => write!(fm, "int({i})"),
            KdlType::Float(f) => write!(fm, "float({f})"),
            KdlType::Str(s) => write!(fm, "string(\"{s}\")"),
            KdlType::Bool(b) => write!(fm, "bool({b})"),
            KdlType::Null => write!(fm, "null"),
        }
    }
}
impl KdlType {
    // TODO: this probably works better if we implemnt Deserialize on template-kdl
    fn into_dyn(self, expected: &TypeInfo) -> Result<DynRefl, ErrTy> {
        use KdlType::*;
        let actual = self.to_string();
        let mismatch = || ErrTy::TypeMismatch { expected: expected.type_name(), actual };
        macro_rules! int2dyn {
            (@opt $int_type:ty, $int_value:expr) => {{
                Ok(Box::new(<$int_type>::try_from($int_value).ok()))
            }};
            ($int_type:ty, $int_value:expr) => {
                <$int_type>::try_from($int_value)
                    .map_err(|_| ErrTy::IntDomain($int_value, any::type_name::<$int_type>()))
                    .map::<DynRefl, _>(|i| Box::new(i))
            };
        }
        macro_rules! null2dyn {
            ($ty_id:expr, $($convert_to:ty,)*) => {
                $(  if $ty_id == TypeId::of::<Option<$convert_to>>() {
                    Ok(Box::new(Option::<$convert_to>::None))
                } else )* {
                    // TODO: meaningfull error message on Option<Foo> where Foo is not primitive
                    Err(mismatch())
                }
            };
        }
        match (self, expected.type_id()) {
            (Int(i), ty) if ty == TypeId::of::<i8>() => int2dyn!(i8, i),
            (Int(i), ty) if ty == TypeId::of::<i16>() => int2dyn!(i16, i),
            (Int(i), ty) if ty == TypeId::of::<i32>() => int2dyn!(i32, i),
            (Int(i), ty) if ty == TypeId::of::<i64>() => Ok(Box::new(i)),
            (Int(i), ty) if ty == TypeId::of::<i128>() => int2dyn!(i128, i),
            (Int(i), ty) if ty == TypeId::of::<isize>() => int2dyn!(isize, i),
            (Int(i), ty) if ty == TypeId::of::<u8>() => int2dyn!(u8, i),
            (Int(i), ty) if ty == TypeId::of::<u16>() => int2dyn!(u16, i),
            (Int(i), ty) if ty == TypeId::of::<u32>() => int2dyn!(u32, i),
            (Int(i), ty) if ty == TypeId::of::<u64>() => int2dyn!(u64, i),
            (Int(i), ty) if ty == TypeId::of::<u128>() => int2dyn!(u128, i),
            (Int(i), ty) if ty == TypeId::of::<usize>() => int2dyn!(usize, i),
            (Int(i), ty) if ty == TypeId::of::<Option<i8>>() => int2dyn!(@opt i8, i),
            (Int(i), ty) if ty == TypeId::of::<Option<i16>>() => int2dyn!(@opt i16, i),
            (Int(i), ty) if ty == TypeId::of::<Option<i32>>() => int2dyn!(@opt i32, i),
            (Int(i), ty) if ty == TypeId::of::<Option<i64>>() => Ok(Box::new(Some(i))),
            (Int(i), ty) if ty == TypeId::of::<Option<i128>>() => int2dyn!(@opt i128, i),
            (Int(i), ty) if ty == TypeId::of::<Option<isize>>() => int2dyn!(@opt isize, i),
            (Int(i), ty) if ty == TypeId::of::<Option<u8>>() => int2dyn!(@opt u8, i),
            (Int(i), ty) if ty == TypeId::of::<Option<u16>>() => int2dyn!(@opt u16, i),
            (Int(i), ty) if ty == TypeId::of::<Option<u32>>() => int2dyn!(@opt u32, i),
            (Int(i), ty) if ty == TypeId::of::<Option<u64>>() => int2dyn!(@opt u64, i),
            (Int(i), ty) if ty == TypeId::of::<Option<u128>>() => int2dyn!(@opt u128, i),
            (Int(i), ty) if ty == TypeId::of::<Option<usize>>() => int2dyn!(@opt usize, i),
            (Int(_), _) => Err(mismatch()),
            (Float(f), ty) if ty == TypeId::of::<f32>() => Ok(Box::new(f as f32)),
            (Float(f), ty) if ty == TypeId::of::<f64>() => Ok(Box::new(f)),
            (Float(f), ty) if ty == TypeId::of::<Option<f32>>() => Ok(Box::new(Some(f as f32))),
            (Float(f), ty) if ty == TypeId::of::<Option<f64>>() => Ok(Box::new(Some(f))),
            (Float(_), _) => Err(mismatch()),
            (Bool(b), ty) if ty == TypeId::of::<bool>() => Ok(Box::new(b)),
            (Bool(b), ty) if ty == TypeId::of::<Option<bool>>() => Ok(Box::new(Some(b))),
            (Bool(_), _) => Err(mismatch()),
            (Str(s), ty) if ty == TypeId::of::<String>() => Ok(Box::new(s)),
            (Str(s), ty) if ty == TypeId::of::<Option<String>>() => Ok(Box::new(Some(s))),
            (Str(_), _) => Err(mismatch()),

            (Null, ty) => null2dyn!(
                ty, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64, bool,
                String,
            ),
        }
    }
}
