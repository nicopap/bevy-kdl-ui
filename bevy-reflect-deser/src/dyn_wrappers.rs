use std::marker::PhantomData;

use bevy_reflect::{
    DynamicList, DynamicMap, DynamicStruct, DynamicTuple, DynamicTupleStruct, ListInfo, Map,
    MapInfo, NamedField, Reflect, Struct, StructInfo, Tuple, TupleInfo, TupleStruct,
    TupleStructInfo, TypeIdentity, TypeInfo, TypeRegistration, TypeRegistry, ValueInfo,
};
use template_kdl::{
    multi_err::{MultiError, MultiErrorTrait, MultiResult},
    multi_try,
    span::Spanned,
};

use crate::{
    err::{ConvResult, ConvertError},
    newtype::ExpectedType,
    visit::{FieldThunk, KdlConcrete, NodeThunkExt},
    DynRefl,
};

pub(crate) type MultiSpan<T> = MultiResult<T, Spanned<ConvertError>>;
pub(crate) trait Infos {
    type DynamicWrapper: Builder<Info = Self>;
    fn name(&self) -> &'static str;
    fn new_dynamic(&self, node: &NodeThunkExt, reg: &TypeRegistry) -> MultiSpan<DynRefl> {
        Self::DynamicWrapper::new_dynamic(self, node, reg)
    }
}
trait FromInfo<I> {
    fn from_info(i: I) -> Self;
}
macro_rules! impl_infos {
    ($ty_name:ty, $field:ty, $dynamic:ty) => {
        impl Infos for $ty_name {
            type DynamicWrapper = Wrapper<$field, $ty_name, $dynamic>;
            fn name(&self) -> &'static str {
                self.id().type_name()
            }
        }
        impl<'i> FromInfo<&'i $ty_name> for $dynamic {
            fn from_info(_: &'i $ty_name) -> Self {
                <$dynamic as Default>::default()
            }
        }
    };
}
pub(crate) fn new_dynamic_anonstruct(
    info: &StructInfo,
    node: &NodeThunkExt,
    reg: &TypeRegistry,
) -> MultiSpan<DynRefl> {
    Wrapper::<(), _, AnonDynamicStruct>::new_dynamic(info, node, reg)
}
impl_infos! {MapInfo, String, DynamicMap}
impl_infos! {StructInfo, String, DynamicStruct}
impl_infos! {ListInfo, (), DynamicList}
impl_infos! {TupleInfo, (), DynamicTuple}
impl_infos! {TupleStructInfo, (), DynamicTupleStruct}
impl Infos for ValueInfo {
    type DynamicWrapper = ValueBuilder;
    fn name(&self) -> &'static str {
        self.id().type_name()
    }
}

pub(crate) fn get_named<'r>(name: &str, reg: &'r TypeRegistry) -> ConvResult<&'r TypeRegistration> {
    reg.get_with_name(name)
        .or_else(|| reg.get_with_short_name(name))
        .ok_or(ConvertError::NoSuchType(name.to_owned()))
}
// Possible failure states:
// * declared is not registered
// * expected is not registered
// * declared is Some and does not match expected
// * Any combination of the above
// * Fatal: only if expected is not registered and (either declared is None or not registered)
pub(crate) fn type_info<'r>(
    reg: &'r TypeRegistry,
    declared: Option<&str>,
    expected: Option<&TypeIdentity>,
) -> MultiResult<ExpectedType<'r>, ConvertError> {
    let mut errors = MultiError::default();
    let declared = declared.and_then(|name| errors.optionally(get_named(name, reg)));
    let no_such = |e: &TypeIdentity| ConvertError::NoSuchType(e.type_name().to_owned());
    let get_from_reg =
        |e: &TypeIdentity| errors.optionally(reg.get(e.type_id()).ok_or_else(|| no_such(e)));
    match (declared, expected.and_then(get_from_reg)) {
        // Both declared and expected are registered, but they are not equal
        // We chose `declared` since that's what is in the file, so we expect that
        // the rest of the file uses the declaredly stated type.
        (Some(declared), Some(expected)) if declared.type_id() != expected.type_id() => {
            let expected = expected.name().to_owned();
            let actual = declared.name().to_owned();
            errors.add_error(ConvertError::TypeMismatch { expected, actual });
            errors.into_result(ExpectedType::new(declared, reg))
        }
        // Either declared was not provided, or it was not registered (in which case
        // the error is already in `errors`) or it was provided, registered and matched
        // expected. And expected is registered
        (Some(_) | None, Some(expected)) => errors.into_result(ExpectedType::new(expected, reg)),
        // Either declared was not provided, or it was not registered (in which case
        // the error is already in `errors`) and expected is not registered
        // NOTE: This is the only Fatal error preventing any validation of what's inside.
        (None, None) => errors.into_errors(ConvertError::UntypedTupleField),
        // declared type exists, but is not equal to expected one, and the
        // expected one is not registered. This is an error, but we continue,
        // hoping to be useful
        (Some(declared), None) => errors.into_result(ExpectedType::new(declared, reg)),
    }
}

type Tid = TypeIdentity;
pub(crate) trait Primitive {
    type Field;
    type Info: Infos;
    fn set_name(&mut self, name: String);
    fn add_boxed(&mut self, field: Self::Field, boxed: DynRefl) -> ConvResult<()>;
    fn expected<'i>(&self, at_field: &Self::Field, info: &'i Self::Info) -> ConvResult<&'i Tid>;
    fn validate(&self, info: &Self::Info) -> ConvResult<()>;
    fn reflect(self) -> Box<dyn Reflect>;
}

impl Primitive for DynamicMap {
    type Field = String;
    type Info = MapInfo;
    fn add_boxed(&mut self, field: String, boxed: DynRefl) -> ConvResult<()> {
        if self.get(&field).is_some() {
            return Err(ConvertError::MultipleSameField { name: self.name().to_owned(), field });
        }
        let box_field = Box::new(field.clone());
        self.insert_boxed(box_field, boxed);
        Ok(())
    }
    fn expected<'i>(&self, _: &String, info: &'i MapInfo) -> ConvResult<&'i Tid> {
        Ok(info.value())
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name);
    }
    fn validate(&self, _: &Self::Info) -> ConvResult<()> {
        Ok(())
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self)
    }
}
impl Primitive for DynamicStruct {
    type Field = String;
    type Info = StructInfo;
    fn add_boxed(&mut self, field: String, boxed: DynRefl) -> ConvResult<()> {
        if self.field(&field).is_some() {
            return Err(ConvertError::MultipleSameField { name: self.name().to_owned(), field });
        }
        self.insert_boxed(&field, boxed);
        Ok(())
    }
    fn expected<'i>(&self, field: &String, info: &'i Self::Info) -> ConvResult<&'i Tid> {
        let name_type =
            |field: &NamedField| (field.name().clone().into_owned(), field.id().type_id());
        let err = || ConvertError::NoSuchStructField {
            name: info.name().to_owned(),
            available: info.iter().map(name_type).collect(),
            requested: field.clone(),
        };
        info.field(&*field).ok_or_else(err).map(|f| f.id())
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name);
    }
    fn validate(&self, info: &Self::Info) -> ConvResult<()> {
        let actual = self.field_len() as u8;
        let expected = info.field_len() as u8;
        if actual != expected {
            let name = info.name();
            // TODO(reporting): find name of missing fields and add them to error
            let expected: Vec<_> = info.iter().map(|t| t.name().clone().into_owned()).collect();
            let is_missing = |n| self.field(n).is_none();
            let missing = expected
                .iter()
                .enumerate()
                .filter_map(|(i, n)| is_missing(n).then(|| i as u8))
                .collect();
            Err(ConvertError::NotEnoughStructFields { name, missing, expected })
        } else {
            Ok(())
        }
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self)
    }
}
struct AnonDynamicStruct(DynamicStruct, StructInfo);

impl AnonDynamicStruct {
    fn new(info: &StructInfo) -> Self {
        Self(DynamicStruct::default(), info.clone())
    }
}
impl<'i> FromInfo<&'i StructInfo> for AnonDynamicStruct {
    fn from_info(i: &'i StructInfo) -> Self {
        Self::new(i)
    }
}
impl Primitive for AnonDynamicStruct {
    type Field = ();
    type Info = StructInfo;
    fn add_boxed(&mut self, _: (), boxed: DynRefl) -> ConvResult<()> {
        let next_index = self.0.field_len();
        let next_field = self.1.field_at(next_index).unwrap();
        self.0.insert_boxed(next_field.name(), boxed);
        Ok(())
    }
    fn expected<'i>(&self, _: &(), info: &'i Self::Info) -> ConvResult<&'i Tid> {
        let requested = self.0.field_len();
        let err = || ConvertError::TooManyTupleStructFields {
            name: info.name(),
            actual: info.field_len() as u8,
            requested: requested as u8,
        };
        info.field_at(requested).ok_or_else(err).map(|f| f.id())
    }
    fn set_name(&mut self, name: String) {
        self.0.set_name(name);
    }
    fn validate(&self, info: &Self::Info) -> ConvResult<()> {
        // The only possible error here is that there are not enough fields, since we
        // already check for too many, and we assume the correct types are provided.
        let actual = self.0.field_len() as u8;
        let expected = info.field_len() as u8;
        if actual != expected {
            // TODO(reporting): Have a variant where the type name is stored
            Err(ConvertError::NotEnoughTupleFields { actual, expected })
        } else {
            Ok(())
        }
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self.0)
    }
}
impl Primitive for DynamicList {
    type Field = ();
    type Info = ListInfo;
    fn add_boxed(&mut self, _: (), boxed: DynRefl) -> ConvResult<()> {
        self.push_box(boxed);
        Ok(())
    }
    fn expected<'i>(&self, _: &(), info: &'i Self::Info) -> ConvResult<&'i Tid> {
        Ok(info.item())
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name);
    }
    fn validate(&self, _: &Self::Info) -> ConvResult<()> {
        Ok(())
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self)
    }
}
impl Primitive for DynamicTuple {
    type Field = ();
    type Info = TupleInfo;
    fn add_boxed(&mut self, _: (), boxed: DynRefl) -> ConvResult<()> {
        self.insert_boxed(boxed);
        Ok(())
    }
    fn expected<'i>(&self, _: &(), info: &'i Self::Info) -> ConvResult<&'i Tid> {
        let requested = self.field_len();
        let err = || ConvertError::TooManyTupleFields {
            actual: info.field_len() as u8,
            requested: requested as u8,
        };
        info.field_at(requested).ok_or_else(err).map(|f| f.id())
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name);
    }
    fn validate(&self, info: &Self::Info) -> ConvResult<()> {
        // The only possible error here is that there are not enough fields, since we
        // already check for too many, and we assume the correct types are provided.
        let actual = self.field_len() as u8;
        let expected = info.field_len() as u8;
        if actual != expected {
            Err(ConvertError::NotEnoughTupleFields { actual, expected })
        } else {
            Ok(())
        }
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self)
    }
}
impl Primitive for DynamicTupleStruct {
    type Field = ();
    type Info = TupleStructInfo;
    fn add_boxed(&mut self, _: (), boxed: DynRefl) -> ConvResult<()> {
        self.insert_boxed(boxed);
        Ok(())
    }
    fn expected<'i>(&self, _: &(), info: &'i Self::Info) -> ConvResult<&'i Tid> {
        let requested = self.field_len();
        let err = || ConvertError::TooManyTupleStructFields {
            name: info.name(),
            actual: info.field_len() as u8,
            requested: requested as u8,
        };
        info.field_at(requested).ok_or_else(err).map(|f| f.id())
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name);
    }
    fn validate(&self, info: &Self::Info) -> ConvResult<()> {
        // The only possible error here is that there are not enough fields, since we
        // already check for too many, and we assume the correct types are provided.
        let actual = self.field_len() as u8;
        let expected = info.field_len() as u8;
        if actual != expected {
            // TODO(reporting): Have a variant where the type name is stored
            Err(ConvertError::NotEnoughTupleFields { actual, expected })
        } else {
            Ok(())
        }
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self)
    }
}

pub(crate) trait Builder: Sized {
    type Info: Infos;
    fn new(expected: &Self::Info) -> Self;
    fn add_field(&mut self, field: FieldThunk, reg: &TypeRegistry) -> MultiSpan<()>;
    fn complete(self) -> MultiResult<DynRefl, ConvertError>;
    fn new_dynamic(
        expected: &Self::Info,
        node: &NodeThunkExt,
        reg: &TypeRegistry,
    ) -> MultiSpan<DynRefl> {
        let mut errors = MultiError::default();
        let mut builder = Self::new(expected);
        for field in node.fields() {
            let _ = errors.optionally(builder.add_field(field, reg));
        }
        builder.complete().map_err_span(node.span()).combine(errors)
    }
}
pub(crate) struct ValueBuilder;
impl Builder for ValueBuilder {
    type Info = ValueInfo;
    fn new(_: &Self::Info) -> Self {
        unreachable!("This should never be called")
    }
    fn add_field(&mut self, _: FieldThunk, _: &TypeRegistry) -> MultiSpan<()> {
        unreachable!("This should never be called")
    }
    fn complete(self) -> MultiResult<DynRefl, ConvertError> {
        unreachable!("This should never be called")
    }
    fn new_dynamic(
        expected: &Self::Info,
        node: &NodeThunkExt,
        _: &TypeRegistry,
    ) -> MultiSpan<DynRefl> {
        if let Some(Spanned(span, value)) = node.first_argument() {
            KdlConcrete::from(value.clone())
                .into_dyn(&TypeInfo::Value(expected.clone()))
                .map_err(|e| Spanned(span, e))
                .into()
        } else {
            let span = node.span();
            let err = ConvertError::NoValuesInNode(expected.name());
            MultiResult::Err(vec![Spanned(span, err)])
        }
    }
}

pub(crate) struct Wrapper<F, I, T> {
    acc: T,
    info: I,
    // This exists so that it's possible to implement Builder separately for
    // wrappers wrapping Field=() and Field=String.
    _f: PhantomData<F>,
}
impl<T> Builder for Wrapper<(), T::Info, T>
where
    T: Primitive<Field = ()> + for<'a> FromInfo<&'a T::Info>,
    T::Info: Clone,
{
    type Info = T::Info;
    fn new(expected: &Self::Info) -> Self {
        let mut acc = T::from_info(expected);
        acc.set_name(expected.name().to_owned());
        Self { acc, info: expected.clone(), _f: PhantomData }
    }
    fn add_field(&mut self, field: FieldThunk, reg: &TypeRegistry) -> MultiSpan<()> {
        let mut errors = MultiError::default();
        let opt_ty = field.ty.or(field.name);
        // In case we have a name or declared type, we use their span for the type
        // error. Otherwise, we use the whole thunk's span.
        let ty_span = opt_ty.map_or_else(|| field.span(), |t| t.0);
        let expected = self.acc.expected(&(), &self.info);
        let expected = errors.optionally(expected.map_err(|e| Spanned(ty_span, e)));
        let try_ty = type_info(reg, opt_ty.map(|t| t.1), expected).map_err_span(ty_span);
        let ty = multi_try!(errors, try_ty);
        let value = multi_try!(errors, ty.into_dyn(field, reg));
        if let Err(err) = self.acc.add_boxed((), value) {
            errors.add_error(Spanned(ty_span, err));
        }
        errors.into_result(())
    }
    fn complete(self) -> MultiResult<DynRefl, ConvertError> {
        let mut errors = MultiError::default();
        let _ = errors.optionally(self.acc.validate(&self.info));
        errors.into_result(self.acc.reflect())
    }
}
impl<T> Builder for Wrapper<String, T::Info, T>
where
    T: Primitive<Field = String> + for<'a> FromInfo<&'a T::Info>,
    T::Info: Clone,
{
    type Info = T::Info;
    fn new(expected: &T::Info) -> Self {
        let mut acc = T::from_info(expected);
        acc.set_name(expected.name().to_owned());
        Self { acc, info: expected.clone(), _f: PhantomData }
    }
    fn add_field(&mut self, field: FieldThunk, reg: &TypeRegistry) -> MultiSpan<()> {
        let mut errors = MultiError::default();
        if let Some(name) = field.name {
            let expect = |n: &str| self.acc.expected(&n.to_owned(), &self.info);
            let expected = name.map_err(expect);
            let expected = errors.optionally(expected);
            let opt_ty = field.ty;
            // In case we have a declared type, we use their span for the type
            // error. Otherwise, we use the name (which incidentally is required)
            let ty_span = opt_ty.map_or(name.0, |t| t.0);
            let try_ty = type_info(reg, opt_ty.map(|t| t.1), expected).map_err_span(ty_span);
            let ty = multi_try!(errors, try_ty);
            let value = multi_try!(errors, ty.into_dyn(field, reg));
            if let Err(err) = self.acc.add_boxed(name.1.to_owned(), value) {
                errors.add_error(Spanned(ty_span, err));
            }
        } else {
            let err = ConvertError::UnnamedMapField { name: self.info.name().to_owned() };
            errors.add_error(Spanned(field.span(), err));
        }
        errors.into_result(())
    }
    fn complete(self) -> MultiResult<DynRefl, ConvertError> {
        let mut errors = MultiError::default();
        let _ = errors.optionally(self.acc.validate(&self.info));
        errors.into_result(self.acc.reflect())
    }
}
