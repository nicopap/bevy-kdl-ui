use std::marker::PhantomData;

use bevy_reflect::{
    DynamicList, DynamicMap, DynamicStruct, DynamicTuple, DynamicTupleStruct, ListInfo, Map,
    MapInfo, NamedField, Reflect, Struct, StructInfo, Tuple, TupleInfo, TupleStruct,
    TupleStructInfo, TypeInfo, TypeRegistry,
};
use multierr_span::{Span, Spanned};
use template_kdl::{
    multi_err::{MultiError, MultiErrorTrait, MultiResult},
    multi_try,
    navigate::{Navigable, Sstring, ThunkField, Value},
};

use crate::{
    err::{ConvResult, ErrorType as ErrTy, ErrorType::GenericUnsupported as TODO, MResult},
    newtype, DynRefl,
};

type Reg = TypeRegistry;
type FieldIter = Box<dyn Iterator<Item = Field>>;
type Field = ThunkField;
trait Infos {
    type DynamicWrapper: Builder<Info = Self>;
    fn name(&self) -> &'static str;
    fn new_dynamic(&self, node: FieldIter, span: Span, reg: &Reg) -> MResult<DynRefl> {
        Self::DynamicWrapper::new_dynamic(self, node, span, reg)
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
                self.type_name()
            }
        }
        impl<'i> FromInfo<&'i $ty_name> for $dynamic {
            fn from_info(_: &'i $ty_name) -> Self {
                <$dynamic as Default>::default()
            }
        }
    };
}
impl_infos! {MapInfo, Sstring, DynamicMap}
impl_infos! {StructInfo, Sstring, DynamicStruct}
impl_infos! {ListInfo, Span, DynamicList}
impl_infos! {TupleInfo, Span, DynamicTuple}
impl_infos! {TupleStructInfo, Span, DynamicTupleStruct}

pub(crate) fn from_expected(info: Option<&TypeInfo>, field: &Field, reg: &Reg) -> MResult<DynRefl> {
    use TypeInfo::{List, Map, Struct, Tuple, TupleStruct};
    let span = field.span();
    let is_first_named = field.is_first_named();
    let fields = field.value().unwrap_list();
    // TODO(reporting): have different return error types for the `new_dynamic`
    // stuff, and collect them so that you can report them together for errors
    // in the style "is missing fields XYZ" and avoid spamming errors
    match info {
        None => AnonTupleInfo.new_dynamic(fields, span, reg),
        Some(Map(v)) if !is_first_named => PairMapBuilder::new_dynamic(v, fields, span, reg),
        Some(Map(v)) => v.new_dynamic(fields, span, reg),
        Some(List(v)) => v.new_dynamic(fields, span, reg),
        Some(Tuple(v)) => v.new_dynamic(fields, span, reg),
        // Some(Tvalue(v)) => v.new_dynamic(fields, span, reg),
        Some(Struct(v)) if is_first_named => v.new_dynamic(fields, span, reg),
        Some(Struct(v)) => Wrapper::<_, _, AnonDynamicStruct>::new_dynamic(v, fields, span, reg),
        Some(TupleStruct(v)) => v.new_dynamic(fields, span, reg),
        Some(_) => {
            let msg = format!("cannot turn field into type: {field:?} \n {info:?}");
            MultiResult::Err(vec![TODO(msg).spanned(field)])
        }
    }
}
trait Primitive {
    type Field;
    type Info: Infos;
    fn set_name(&mut self, name: String);
    fn add_boxed(&mut self, field: Self::Field, boxed: DynRefl) -> ConvResult<()>;
    fn expected(&self, at_field: &Self::Field, info: &Self::Info) -> ConvResult<&'static str>;
    fn validate(&self, info: &Self::Info) -> Result<(), ErrTy>;
    fn reflect(self) -> Box<dyn Reflect>;
}

/// A Builder for maps declared as a pair of complex types rather than
/// `name value` style.
struct PairMapBuilder(DynamicMap, MapInfo);
impl Builder for PairMapBuilder {
    type Info = MapInfo;

    fn new(expected: &Self::Info) -> Self {
        Self(DynamicMap::default(), expected.clone())
    }
    fn add_field(&mut self, field: Field, reg: &Reg) -> MResult<()> {
        let mut err = MultiError::default();
        let field_count = match field.value_count() {
            Value::Bare(_) => 1,
            Value::List(i) => i,
        };
        if field_count != 2 {
            let err = ErrTy::PairMapNotPair(field_count as u8).spanned(&field);
            return MultiResult::Err(vec![err]);
        }
        let key_name = self.1.key_type_name();
        let value_name = self.1.value_type_name();
        let mut fields = field.value().unwrap_list();
        let key = newtype::make_dyn(reg, Some(key_name), fields.next().unwrap());
        let value = newtype::make_dyn(reg, Some(value_name), fields.next().unwrap());
        self.0
            .insert_boxed(multi_try!(err, key), multi_try!(err, value));
        err.into_result(())
    }
    fn complete(self) -> MultiResult<DynRefl, ErrTy> {
        MultiResult::Ok(Box::new(self.0))
    }
}

impl Primitive for DynamicMap {
    type Field = Sstring;
    type Info = MapInfo;
    fn add_boxed(&mut self, field: Self::Field, boxed: DynRefl) -> ConvResult<()> {
        let field_name = Box::new(field.to_string());
        if self.get(&*field_name).is_some() {
            let name = self.name().to_owned();
            return Err(ErrTy::MultipleSameField { name, field: *field_name }.spanned(&field));
        }
        self.insert_boxed(field_name, boxed);
        Ok(())
    }
    fn expected(&self, _: &Self::Field, info: &MapInfo) -> ConvResult<&'static str> {
        Ok(info.value_type_name())
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name);
    }
    fn validate(&self, _: &Self::Info) -> Result<(), ErrTy> {
        Ok(())
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self)
    }
}
impl Primitive for DynamicList {
    type Field = Span;
    type Info = ListInfo;
    fn add_boxed(&mut self, _: Span, boxed: DynRefl) -> ConvResult<()> {
        self.push_box(boxed);
        Ok(())
    }
    fn expected(&self, _: &Span, info: &Self::Info) -> ConvResult<&'static str> {
        Ok(info.item_type_name())
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name);
    }
    fn validate(&self, _: &Self::Info) -> Result<(), ErrTy> {
        Ok(())
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self)
    }
}
impl Primitive for DynamicStruct {
    type Field = Sstring;
    type Info = StructInfo;
    fn add_boxed(&mut self, field: Sstring, boxed: DynRefl) -> ConvResult<()> {
        if self.field(&field).is_some() {
            let name = self.name().to_owned();
            let field_name = field.to_string();
            return Err(ErrTy::MultipleSameField { name, field: field_name }.spanned(&field));
        }
        self.insert_boxed(&field, boxed);
        Ok(())
    }
    fn expected(&self, field: &Sstring, info: &Self::Info) -> ConvResult<&'static str> {
        let name_type = |field: &NamedField| (field.name().to_owned(), field.type_name());
        let err = || {
            ErrTy::NoSuchStructField {
                name: info.name(),
                available: info.iter().map(name_type).collect(),
                requested: field.to_string(),
            }
            .spanned(field)
        };
        info.field(field).ok_or_else(err).map(|f| f.type_name())
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name);
    }
    fn validate(&self, info: &Self::Info) -> Result<(), ErrTy> {
        let actual = self.field_len() as u8;
        let expected = info.field_len() as u8;
        if actual != expected {
            let name = info.name();
            // TODO(reporting): find name of missing fields and add them to error
            let expected: Vec<_> = info.iter().map(|t| t.name().to_owned()).collect();
            let is_missing = |n| self.field(n).is_none();
            let missing = expected
                .iter()
                .enumerate()
                .filter_map(|(i, n)| is_missing(n).then_some(i as u8))
                .collect();
            Err(ErrTy::NotEnoughStructFields { name, missing, expected })
        } else {
            Ok(())
        }
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self)
    }
}

// TODO(??): consider explicit declaration of tuple length
struct AnonTupleInfo;
impl Infos for AnonTupleInfo {
    type DynamicWrapper = AnonTupleBuilder;
    fn name(&self) -> &'static str {
        "Tuple"
    }
}
struct AnonTupleBuilder(DynamicTuple);
impl Builder for AnonTupleBuilder {
    type Info = AnonTupleInfo;

    fn new(_: &Self::Info) -> Self {
        Self(DynamicTuple::default())
    }

    fn add_field(&mut self, field: Field, reg: &Reg) -> MResult<()> {
        let mut errors = MultiError::default();
        let value = multi_try!(errors, newtype::make_named_dyn(reg, None, field));
        self.0.insert_boxed(value);
        errors.into_result(())
    }

    fn complete(self) -> MultiResult<DynRefl, ErrTy> {
        MultiResult::Ok(Box::new(self.0))
    }
}

/// A Builder for structs declared with anonymous fields.
struct AnonDynamicStruct(DynamicStruct, StructInfo);
impl<'i> FromInfo<&'i StructInfo> for AnonDynamicStruct {
    fn from_info(i: &'i StructInfo) -> Self {
        Self(DynamicStruct::default(), i.clone())
    }
}
impl Primitive for AnonDynamicStruct {
    type Field = Span;
    type Info = StructInfo;
    fn add_boxed(&mut self, _: Span, boxed: DynRefl) -> ConvResult<()> {
        let next_index = self.0.field_len();
        let next_field = self.1.field_at(next_index).unwrap();
        self.0.insert_boxed(next_field.name(), boxed);
        Ok(())
    }
    fn expected(&self, span: &Span, info: &Self::Info) -> ConvResult<&'static str> {
        let requested = self.0.field_len() as u8;
        let err = || {
            let actual = info.field_len() as u8;
            ErrTy::TooManyFields { name: info.name(), actual, requested }.spanned(span)
        };
        info.field_at(requested as usize)
            .ok_or_else(err)
            .map(|f| f.type_name())
    }
    fn set_name(&mut self, name: String) {
        self.0.set_name(name);
    }
    fn validate(&self, info: &Self::Info) -> Result<(), ErrTy> {
        // The only possible error here is that there are not enough fields, since we
        // already check for too many, and we assume the correct types are provided.
        let actual = self.0.field_len() as u8;
        let expected = info.field_len() as u8;
        if actual != expected {
            // TODO(reporting): Have a variant where the type name is stored
            Err(ErrTy::NotEnoughTupleFields { actual, expected })
        } else {
            Ok(())
        }
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self.0)
    }
}
impl Primitive for DynamicTuple {
    type Field = Span;
    type Info = TupleInfo;
    fn add_boxed(&mut self, _: Span, boxed: DynRefl) -> ConvResult<()> {
        self.insert_boxed(boxed);
        Ok(())
    }
    fn expected(&self, span: &Span, info: &Self::Info) -> ConvResult<&'static str> {
        let requested = self.field_len() as u8;
        let err = || {
            let actual = info.field_len() as u8;
            ErrTy::TooManyFields { name: "Tuple", actual, requested }.spanned(span)
        };
        info.field_at(requested as usize)
            .ok_or_else(err)
            .map(|f| f.type_name())
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name);
    }
    fn validate(&self, info: &Self::Info) -> Result<(), ErrTy> {
        // The only possible error here is that there are not enough fields, since we
        // already check for too many, and we assume the correct types are provided.
        let actual = self.field_len() as u8;
        let expected = info.field_len() as u8;
        if actual != expected {
            Err(ErrTy::NotEnoughTupleFields { actual, expected })
        } else {
            Ok(())
        }
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self)
    }
}
impl Primitive for DynamicTupleStruct {
    type Field = Span;
    type Info = TupleStructInfo;
    fn add_boxed(&mut self, _: Span, boxed: DynRefl) -> ConvResult<()> {
        self.insert_boxed(boxed);
        Ok(())
    }
    fn expected(&self, span: &Span, info: &Self::Info) -> ConvResult<&'static str> {
        let requested = self.field_len() as u8;
        let err = || {
            let actual = info.field_len() as u8;
            ErrTy::TooManyFields { name: info.name(), actual, requested }.spanned(span)
        };
        info.field_at(requested as usize)
            .ok_or_else(err)
            .map(|f| f.type_name())
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name);
    }
    fn validate(&self, info: &Self::Info) -> Result<(), ErrTy> {
        // The only possible error here is that there are not enough fields, since we
        // already check for too many, and we assume the correct types are provided.
        let actual = self.field_len() as u8;
        let expected = info.field_len() as u8;
        if actual != expected {
            // TODO(reporting): Have a variant where the type name is stored
            Err(ErrTy::NotEnoughTupleFields { actual, expected })
        } else {
            Ok(())
        }
    }
    fn reflect(self) -> Box<dyn Reflect> {
        Box::new(self)
    }
}

trait Builder: Sized {
    type Info: Infos;
    fn new(expected: &Self::Info) -> Self;
    fn add_field(&mut self, field: Field, reg: &Reg) -> MResult<()>;
    fn complete(self) -> MultiResult<DynRefl, ErrTy>;
    fn new_dynamic(
        expected: &Self::Info,
        value: FieldIter,
        span: Span,
        reg: &Reg,
    ) -> MResult<DynRefl> {
        let mut errors = MultiError::default();
        let mut builder = Self::new(expected);
        for field in value {
            let _ = errors.optionally(builder.add_field(field, reg));
        }
        builder
            .complete()
            .map_err(|e| e.spanned(&span))
            .combine(errors)
    }
}

fn add_expected<P, T, I>(field: Field, acc: &mut P, name: T, reg: &Reg, info: &I) -> MResult<()>
where
    P: Primitive<Field = T, Info = I>,
{
    let mut errors = MultiError::default();
    let expected = errors.optionally(acc.expected(&name, info));
    let value = multi_try!(errors, newtype::make_dyn(reg, expected, field));
    let _ = errors.optionally(acc.add_boxed(name, value));
    errors.into_result(())
}
struct Wrapper<F, I, T> {
    acc: T,
    info: I,
    // This exists so that it's possible to implement Builder separately for
    // wrappers wrapping Field=() and Field=String.
    _f: PhantomData<F>,
}
impl<T> Builder for Wrapper<Span, T::Info, T>
where
    T: Primitive<Field = Span> + for<'a> FromInfo<&'a T::Info>,
    T::Info: Clone,
{
    type Info = T::Info;
    fn new(expected: &Self::Info) -> Self {
        let mut acc = T::from_info(expected);
        acc.set_name(expected.name().to_owned());
        Self { acc, info: expected.clone(), _f: PhantomData }
    }
    fn add_field(&mut self, field: Field, reg: &Reg) -> MResult<()> {
        let span = field.span();
        add_expected(field, &mut self.acc, span, reg, &self.info)
    }
    fn complete(self) -> MultiResult<DynRefl, ErrTy> {
        let mut errors = MultiError::default();
        let _ = errors.optionally(self.acc.validate(&self.info));
        errors.into_result(self.acc.reflect())
    }
}

impl<T> Builder for Wrapper<Sstring, T::Info, T>
where
    T: Primitive<Field = Sstring> + for<'a> FromInfo<&'a T::Info>,
    T::Info: Clone,
{
    type Info = T::Info;
    fn new(expected: &Self::Info) -> Self {
        let mut acc = T::from_info(expected);
        acc.set_name(expected.name().to_owned());
        Self { acc, info: expected.clone(), _f: PhantomData }
    }
    fn add_field(&mut self, field: Field, reg: &Reg) -> MResult<()> {
        let span = field.span();
        if let Some(name) = field.name() {
            add_expected(field, &mut self.acc, name, reg, &self.info)
        } else {
            let mut errors = MultiError::default();
            errors.add_error(ErrTy::UnnamedMapField { name: self.info.name() }.spanned(&span));
            errors.into_result(())
        }
    }
    fn complete(self) -> MultiResult<DynRefl, ErrTy> {
        let mut errors = MultiError::default();
        let _ = errors.optionally(self.acc.validate(&self.info));
        errors.into_result(self.acc.reflect())
    }
}
