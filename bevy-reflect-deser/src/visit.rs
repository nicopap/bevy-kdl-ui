use std::any;
use std::{any::TypeId, fmt};

use kdl::KdlDocument;

use bevy_reflect::{
    DynamicStruct, DynamicTuple, DynamicTupleStruct, TypeIdentity, TypeInfo, TypeRegistry,
};
use kdl::KdlValue;
use template_kdl::{
    multi_err::MultiResult,
    span::{Span, Spanned},
    template::{EntryThunk, NodeThunk},
};

use crate::dyn_wrappers::{get_named, Infos};
use crate::err::{
    ConvResult, ConvertError, ConvertError::GenericUnsupported as TODO, ConvertErrors,
};
use crate::{ConvertResult, DynRefl};

pub fn convert_doc(doc: &KdlDocument, registry: &TypeRegistry) -> ConvertResult<DynRefl> {
    let doc_repr = doc.to_string();
    // TODO(errs)
    let (doc, errs) = template_kdl::read_document(doc);
    let doc = doc.unwrap(); // TODO(unwrap)
    let expected = get_named(doc.name().1, registry);
    let expected = expected.unwrap().type_info(); // TODO(unwrap)
    NodeThunkExt(doc)
        .into_dyn(expected, registry)
        .into_result()
        .map_err(|e| ConvertErrors::new(doc_repr, e))
}

/// A proxy for [`KdlValue`] that doesn't care about the format of declaration.
pub(crate) enum KdlConcrete {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Null,
}
impl From<KdlValue> for KdlConcrete {
    fn from(value: KdlValue) -> Self {
        use KdlValue::{
            Base10, Base10Float, Base16, Base2, Base8, Bool, Null, RawString, String as VString,
        };
        match value {
            Base10(i) | Base2(i) | Base16(i) | Base8(i) => Self::Int(i),
            Base10Float(f) => Self::Float(f),
            VString(s) | RawString(s) => Self::Str(s),
            Bool(b) => Self::Bool(b),
            Null => Self::Null,
        }
    }
}
impl fmt::Display for KdlConcrete {
    fn fmt(&self, fm: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(i) => write!(fm, "int({i})"),
            Self::Float(f) => write!(fm, "float({f})"),
            Self::Str(s) => write!(fm, "string(\"{s}\")"),
            Self::Bool(b) => write!(fm, "bool({b})"),
            Self::Null => write!(fm, "null"),
        }
    }
}
impl KdlConcrete {
    /// Try to get a DynRefl corresponding to provided `handle` type from this
    /// [`KdlConcrete`].
    ///
    /// Inspects recursively newtype-style structs (aka structs will a single field) if
    /// `handle` proves to be such a thing.
    ///
    /// This is useful to inline in entry position newtype struct.
    pub(crate) fn dyn_value(
        self,
        handle: &TypeIdentity,
        reg: &TypeRegistry,
    ) -> ConvResult<DynRefl> {
        self.dyn_value_newtypes(handle, reg, Vec::new())
    }
    /// Recursively resolves newtype structs attempting to summarize them into a primitive
    /// type.
    fn dyn_value_newtypes(
        self,
        handle: &TypeIdentity,
        reg: &TypeRegistry,
        mut wrappers: Vec<&'static str>,
    ) -> ConvResult<DynRefl> {
        use TypeInfo::{Struct, Tuple, TupleStruct, Value};
        wrappers.push(handle.type_name());
        let mismatch = |actual| {
            || ConvertError::TypeMismatch {
                expected: if wrappers.len() == 1 {
                    wrappers[0].to_string()
                } else {
                    format!("any of {}", wrappers.join(", "))
                },
                actual,
            }
        };
        macro_rules! create_dynamic {
            (@insert DynamicStruct, $field:expr, $ret:expr, $val:expr) => (
                $ret.insert_boxed($field.name(), $val)
            );
            (@insert $_1:ident, $_2:expr, $ret:expr, $val:expr) => ( $ret.insert_boxed($val) );
            ($dynamic_kind:ident, $info:expr) => {{
                // unwrap: we just checked that length == 1
                let field = $info.field_at(0).unwrap();
                let field_value = self.dyn_value_newtypes(field.id(), reg, wrappers)?;
                let mut ret = $dynamic_kind::default();
                ret.set_name(handle.type_name().to_string());
                create_dynamic!(@insert $dynamic_kind, field, ret, field_value);
                Ok(Box::new(ret))
            }}
        }
        match reg.get_type_info(handle.type_id()) {
            None => Err(ConvertError::NoSuchType(handle.type_name().to_string())),
            Some(Struct(info)) if info.field_len() == 0 => {
                match (self, reg.get(handle.type_id())) {
                    (Self::Str(s), Some(reg)) if reg.short_name() == s || reg.name() == s => {
                        let mut ret = DynamicStruct::default();
                        ret.set_name(handle.type_name().to_string());
                        Ok(Box::new(ret))
                    }
                    (_, None) => Err(ConvertError::NoSuchType(handle.type_name().to_string())),
                    (s, Some(_)) => Err(mismatch(s.to_string())()),
                }
            }
            Some(Struct(i)) if i.field_len() == 1 => create_dynamic!(DynamicStruct, i),
            Some(Tuple(i)) if i.field_len() == 1 => create_dynamic!(DynamicTuple, i),
            Some(TupleStruct(i)) if i.field_len() == 1 => create_dynamic!(DynamicTupleStruct, i),
            Some(Value(info)) => {
                let mismatch = mismatch(self.to_string());
                self.dyn_primitive_value(info.id(), mismatch)
            }
            Some(_) => Err(mismatch(self.to_string())()),
        }
    }
    // TODO: this probably works if we implemnt Deserialize on template-kdl
    /// Converts a raw primitive type into `DynRefl`, making sure they have
    /// the same type as the `handle` provides.
    fn dyn_primitive_value(
        self,
        handle: &TypeIdentity,
        mismatch: impl FnOnce() -> ConvertError,
    ) -> ConvResult<DynRefl> {
        use KdlConcrete::*;
        macro_rules! int2dyn {
            (@opt $int_type:ty, $int_value:expr) => {{
                Ok(Box::new(<$int_type>::try_from($int_value).ok()))
            }};
            ($int_type:ty, $int_value:expr) => {
                <$int_type>::try_from($int_value)
                    .map_err(|_| ConvertError::IntDomain($int_value, any::type_name::<$int_type>()))
                    .map::<DynRefl, _>(|i| Box::new(i))
            };
        }
        let msg = "null values currently cannot be converted into rust types";
        let unsupported = || Err(ConvertError::GenericUnsupported(msg.to_string()));
        match (self, handle.type_id()) {
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

            (Null, _) => unsupported(),
        }
    }
}

pub(crate) enum ValueExt<'s> {
    Node(NodeThunkExt<'s>),
    Value(Spanned<&'s KdlValue>),
}
impl<'s> ValueExt<'s> {
    pub(crate) fn into_dyn(
        self,
        expected: &TypeInfo,
        reg: &TypeRegistry,
    ) -> MultiResult<DynRefl, Spanned<ConvertError>> {
        match self {
            ValueExt::Node(node) => node.into_dyn(expected, reg),
            ValueExt::Value(value) => value
                .map_err(|value| KdlConcrete::from(value.clone()).dyn_value(expected.id(), reg))
                .into(),
        }
    }
    fn span(&self) -> Span {
        match self {
            ValueExt::Node(n) => n.span(),
            ValueExt::Value(v) => v.0,
        }
    }
}

pub(crate) struct FieldThunk<'s> {
    pub(crate) ty: Option<Spanned<&'s str>>,
    pub(crate) name: Option<Spanned<&'s str>>,
    pub(crate) value: ValueExt<'s>,
    span: Span,
}

impl<'s> FieldThunk<'s> {
    fn new(
        declared_ty: Option<Spanned<&'s str>>,
        field_name: Option<Spanned<&'s str>>,
        value: ValueExt<'s>,
        span: Span,
    ) -> Self {
        let field_name = field_name.filter(|name| name.1 != "-");
        Self { ty: declared_ty, name: field_name, value, span }
    }
    pub(crate) fn span(&self) -> Span {
        self.span
    }
}

impl<'s> From<EntryThunk<'s>> for FieldThunk<'s> {
    fn from(entry: EntryThunk<'s>) -> Self {
        let span = entry.span();
        let value = ValueExt::Value(entry.value());
        Self::new(entry.ty(), entry.name(), value, span)
    }
}
impl<'s> From<NodeThunk<'s>> for FieldThunk<'s> {
    fn from(node: NodeThunk<'s>) -> Self {
        let span = node.span();
        Self::new(
            node.ty(),
            Some(node.name()),
            ValueExt::Node(NodeThunkExt(node)),
            span,
        )
    }
}

#[derive(Debug)]
pub(crate) struct NodeThunkExt<'s>(NodeThunk<'s>);

impl<'s> NodeThunkExt<'s> {
    fn name(&self) -> Spanned<&'s str> {
        self.0.name()
    }
    // TODO: actual error handling (eg: check there is not more than 1 etc.)
    pub(crate) fn first_argument(&self) -> Option<Spanned<&'s KdlValue>> {
        self.0.entries().next().map(|e| e.value())
    }
    pub(crate) fn fields(&self) -> impl Iterator<Item = FieldThunk<'s>> {
        let entries = self.0.entries().map(Into::into);
        let children = self.0.children().map(Into::into);
        entries.chain(children)
    }
    pub(crate) fn span(&self) -> Span {
        self.0.span()
    }
    fn into_dyn(
        self,
        expected: &TypeInfo,
        reg: &TypeRegistry,
    ) -> MultiResult<DynRefl, Spanned<ConvertError>> {
        use TypeInfo::{List, Map, Struct, Tuple, TupleStruct, Value};
        match expected {
            Map(v) => v.new_dynamic(self, reg),
            List(v) => v.new_dynamic(self, reg),
            Tuple(v) => v.new_dynamic(self, reg),
            Value(v) => v.new_dynamic(self, reg),
            Struct(v) => v.new_dynamic(self, reg),
            TupleStruct(v) => v.new_dynamic(self, reg),
            v => MultiResult::Err(vec![Spanned(
                self.span(),
                TODO(format!("cannot turn node into type: {self:?} and {v:?}")),
            )]),
        }
    }
}
