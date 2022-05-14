use std::any;
use std::{any::TypeId, fmt};

use kdl::KdlDocument;

use bevy_reflect::{TypeInfo, TypeRegistry};
use kdl::KdlValue;
use template_kdl::{
    multi_err::{MultiError, MultiErrorTrait, MultiResult},
    span::{Span, Spanned},
    template::{EntryThunk, NodeThunk},
};

use crate::{
    dyn_wrappers::{new_dynamic_anonstruct, type_info, Infos},
    err::{ConvertError, ConvertError::GenericUnsupported as TODO, ConvertErrors},
    newtype::ExpectedType,
    ConvertResult, DynRefl,
};

pub fn convert_doc(doc: &KdlDocument, registry: &TypeRegistry) -> ConvertResult<DynRefl> {
    let mut errors = MultiError::default();
    let doc_repr = doc.to_string();
    let template_map = |err: Spanned<_>| err.map(ConvertError::Template);
    if let Some(doc) = errors.optionally(template_kdl::read_document(doc).map_err(template_map)) {
        let Spanned(name_span, name) = doc.name();
        let expected = type_info(registry, Some(name), None).map_err_span(name_span);
        let node = ValueExt::Node(NodeThunkExt(doc));
        expected
            .combine(errors)
            .and_then(|e| e.into_dyn(node, registry))
            .into_result()
            .map_err(|e| ConvertErrors::new(doc_repr, e))
    } else {
        Err(ConvertErrors::new(doc_repr, errors.errors().to_vec()))
    }
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
    // TODO: this probably works better if we implemnt Deserialize on template-kdl
    pub(crate) fn into_dyn(&self, expected: &TypeInfo) -> Result<DynRefl, ConvertError> {
        use KdlConcrete::*;
        let expected = expected.id();
        let mismatch = || ConvertError::TypeMismatch {
            expected: expected.type_name().to_owned(),
            actual: self.to_string(),
        };
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
            (&Int(i), ty) if ty == TypeId::of::<i8>() => int2dyn!(i8, i),
            (&Int(i), ty) if ty == TypeId::of::<i16>() => int2dyn!(i16, i),
            (&Int(i), ty) if ty == TypeId::of::<i32>() => int2dyn!(i32, i),
            (&Int(i), ty) if ty == TypeId::of::<i64>() => Ok(Box::new(i)),
            (&Int(i), ty) if ty == TypeId::of::<i128>() => int2dyn!(i128, i),
            (&Int(i), ty) if ty == TypeId::of::<isize>() => int2dyn!(isize, i),
            (&Int(i), ty) if ty == TypeId::of::<u8>() => int2dyn!(u8, i),
            (&Int(i), ty) if ty == TypeId::of::<u16>() => int2dyn!(u16, i),
            (&Int(i), ty) if ty == TypeId::of::<u32>() => int2dyn!(u32, i),
            (&Int(i), ty) if ty == TypeId::of::<u64>() => int2dyn!(u64, i),
            (&Int(i), ty) if ty == TypeId::of::<u128>() => int2dyn!(u128, i),
            (&Int(i), ty) if ty == TypeId::of::<usize>() => int2dyn!(usize, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<i8>>() => int2dyn!(@opt i8, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<i16>>() => int2dyn!(@opt i16, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<i32>>() => int2dyn!(@opt i32, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<i64>>() => Ok(Box::new(Some(i))),
            (&Int(i), ty) if ty == TypeId::of::<Option<i128>>() => int2dyn!(@opt i128, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<isize>>() => int2dyn!(@opt isize, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<u8>>() => int2dyn!(@opt u8, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<u16>>() => int2dyn!(@opt u16, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<u32>>() => int2dyn!(@opt u32, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<u64>>() => int2dyn!(@opt u64, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<u128>>() => int2dyn!(@opt u128, i),
            (&Int(i), ty) if ty == TypeId::of::<Option<usize>>() => int2dyn!(@opt usize, i),
            (&Int(_), _) => Err(mismatch()),
            (&Float(f), ty) if ty == TypeId::of::<f32>() => Ok(Box::new(f as f32)),
            (&Float(f), ty) if ty == TypeId::of::<f64>() => Ok(Box::new(f)),
            (&Float(f), ty) if ty == TypeId::of::<Option<f32>>() => Ok(Box::new(Some(f as f32))),
            (&Float(f), ty) if ty == TypeId::of::<Option<f64>>() => Ok(Box::new(Some(f))),
            (&Float(_), _) => Err(mismatch()),
            (&Bool(b), ty) if ty == TypeId::of::<bool>() => Ok(Box::new(b)),
            (&Bool(b), ty) if ty == TypeId::of::<Option<bool>>() => Ok(Box::new(Some(b))),
            (&Bool(_), _) => Err(mismatch()),
            (Str(s), ty) if ty == TypeId::of::<String>() => Ok(Box::new(s.clone())),
            (Str(s), ty) if ty == TypeId::of::<Option<String>>() => Ok(Box::new(Some(s.clone()))),
            (Str(_), _) => Err(mismatch()),

            (Null, ty) => null2dyn!(
                ty, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64, bool,
                String,
            ),
        }
    }
}

pub(crate) enum ValueExt<'s> {
    Node(NodeThunkExt<'s>),
    // TODO: the Value alt should encode expected type
    Value(Spanned<&'s KdlValue>),
}
impl<'s> ValueExt<'s> {
    pub(crate) fn into_dyn<'a>(
        self,
        expected: &'a TypeInfo,
        reg: &'a TypeRegistry,
    ) -> MultiResult<DynRefl, Spanned<ConvertError>> {
        match self {
            ValueExt::Node(node) => node.into_dyn(expected, reg),
            ValueExt::Value(value) => KdlConcrete::from(value.1.clone())
                .into_dyn(expected)
                .map_err(|e| Spanned(value.0, e))
                .into(),
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
    fn is_anon(&self) -> bool {
        self.fields().next().map_or(false, |e| e.name.is_none())
    }
    fn into_dyn(
        &self,
        expected: &TypeInfo,
        reg: &TypeRegistry,
    ) -> MultiResult<DynRefl, Spanned<ConvertError>> {
        use TypeInfo::{List, Map, Struct, Tuple, TupleStruct, Value};
        match expected {
            Map(v) => v.new_dynamic(self, reg),
            List(v) => v.new_dynamic(self, reg),
            Tuple(v) => v.new_dynamic(self, reg),
            Value(v) => v.new_dynamic(self, reg),
            Struct(v) if self.is_anon() => new_dynamic_anonstruct(v, self, reg),
            Struct(v) => v.new_dynamic(self, reg),
            TupleStruct(v) => v.new_dynamic(self, reg),
            v => MultiResult::Err(vec![Spanned(
                self.span(),
                TODO(format!("cannot turn node into type: {self:?} and {v:?}")),
            )]),
        }
    }
}
