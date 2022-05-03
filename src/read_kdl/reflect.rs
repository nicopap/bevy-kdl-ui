use std::fmt;
use std::num::NonZeroU8;

use super::FieldIdentError;
use bevy_reflect::{DynamicStruct, DynamicTupleStruct, Reflect};
use kdl::{KdlIdentifier, KdlValue};

// TODO use NonMaxValue instead of NonZero to get rid of +1 -1
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Position(NonZeroU8);
impl Position {
    pub(super) fn value(&self) -> usize {
        let pos: u8 = self.0.into();
        pos as usize - 1
    }
    pub(super) fn new(position: usize) -> Self {
        // unwrap: Only panics if a struct has more than 255 fields
        let u8: u8 = position.try_into().unwrap();
        Self::new_u8(u8)
    }
    pub(super) fn new_u8(position: u8) -> Self {
        // unwrap: literally cannot panic, as the result of `saturating_add`
        // is in domain [1,255]
        Self(position.saturating_add(1).try_into().unwrap())
    }
}
impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let u8_repr: u8 = self.0.into();
        write!(f, "{}", u8_repr - 1)
    }
}

#[derive(Debug)]
pub(super) enum FieldRef<'a> {
    Implicit,
    Positional(Position),
    Named(&'a str),
}
impl<'a> FieldRef<'a> {
    pub(super) fn from_ident(ident: &'a KdlIdentifier) -> Result<Self, FieldIdentError> {
        let name = ident.value();
        match name.strip_prefix('.').map(str::parse::<u8>) {
            None => Ok(Self::Implicit),
            Some(Ok(index)) => Ok(Self::Positional(Position::new_u8(index))),
            Some(Err(_)) => Ok(Self::Named(name.split_at(1).1)),
        }
    }
}
impl<'a> FieldRef<'a> {
    fn into_owned(self) -> OwnedField {
        match self {
            Self::Named(name) => OwnedField::Named(name.to_owned()),
            Self::Positional(i) => OwnedField::Positional(i),
            Self::Implicit => OwnedField::Implicit,
        }
    }
}
#[derive(Debug)]
pub enum OwnedField {
    Implicit,
    Positional(Position),
    Named(String),
}
impl OwnedField {
    pub(super) fn named(self) -> Option<String> {
        if let Self::Named(s) = self {
            Some(s)
        } else {
            None
        }
    }
}
impl fmt::Display for OwnedField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Implicit => write!(f, "unspecified field"),
            Self::Positional(p) => write!(f, "field {p}"),
            Self::Named(name) => write!(f, "field \"{name}\""),
        }
    }
}

#[derive(Debug)]
pub(super) enum DynamicKdlValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Struct(DynamicKdlType),
}
impl<'a> From<&'a KdlValue> for DynamicKdlValue {
    fn from(kdl_value: &'a KdlValue) -> Self {
        use KdlValue as Kv;
        match kdl_value {
            Kv::RawString(s) | Kv::String(s) => Self::String(s.to_owned()),
            Kv::Base2(i) | Kv::Base8(i) | Kv::Base10(i) | Kv::Base16(i) => Self::Int(*i),
            Kv::Bool(b) => Self::Bool(*b),
            Kv::Base10Float(f) => Self::Float(*f),
            Kv::Null => todo!("handle this horrible failure case"),
        }
    }
}
impl DynamicKdlValue {
    fn into_reflect(self) -> Box<dyn Reflect> {
        match self {
            Self::String(s) => Box::new(s),
            Self::Int(s) => Box::new(s),
            Self::Float(s) => Box::new(s),
            Self::Bool(s) => Box::new(s),
            Self::Struct(dynamic) => dynamic.into_reflect(),
        }
    }
}

#[derive(Debug)]
pub(super) enum DynamicKdlValueRef<'a> {
    Struct(DynamicKdlTypeRef<'a>),
    KdlValue(&'a KdlValue),
}
impl<'a> DynamicKdlValueRef<'a> {
    fn into_owned(self) -> DynamicKdlValue {
        match self {
            Self::KdlValue(value) => value.into(),
            Self::Struct(st) => DynamicKdlValue::Struct(st.into_owned()),
        }
    }
}
#[derive(Debug)]
pub struct DynamicKdlTypeRef<'a> {
    pub(super) name: &'a str,
    pub(super) fields: Vec<DynamicKdlValueRef<'a>>,
    pub(super) fields_spec: Vec<FieldRef<'a>>,
    // TODO
    // fields_spans: Vec<SourceSpan>,
}
impl<'a> DynamicKdlTypeRef<'a> {
    fn into_owned(self) -> DynamicKdlType {
        DynamicKdlType {
            name: self.name.to_owned(),
            fields: self.fields.into_iter().map(|f| f.into_owned()).collect(),
            fields_spec: self
                .fields_spec
                .into_iter()
                .map(|f| f.into_owned())
                .collect(),
        }
    }
}

// TODO: Consider having a enum KdlStructFields that explicitly prevent
// mixing of named and unammed fields.
#[derive(Debug)]
pub struct DynamicKdlType {
    pub(super) name: String,
    pub(super) fields: Vec<DynamicKdlValue>,
    pub(super) fields_spec: Vec<OwnedField>,
}
impl DynamicKdlType {
    pub fn into_struct(self) -> DynamicStruct {
        let mut dynamic = DynamicStruct::default();
        dynamic.set_name(self.name);
        let named_fields = self
            .fields
            .into_iter()
            .zip(self.fields_spec.into_iter())
            .filter_map(|(val, field)| match field {
                OwnedField::Named(name) => Some((name, val)),
                _ => None,
            });
        for (name, value) in named_fields {
            dynamic.insert_boxed(&name, value.into_reflect());
        }
        dynamic
    }
    pub fn into_tuple_struct(self) -> DynamicTupleStruct {
        // TODO: explicit field access with .0 etc.
        use OwnedField::Implicit;
        let Self {
            fields_spec,
            fields,
            name,
        } = self;
        let mut dynamic = DynamicTupleStruct::default();
        dynamic.set_name(name);

        let keep_implicits = |(f, val)| matches!(f, &Implicit).then(|| val);
        let fields = fields_spec
            .iter()
            .zip(fields.into_iter())
            .filter_map(keep_implicits);
        for value in fields {
            dynamic.insert_boxed(value.into_reflect());
        }
        dynamic
    }
    pub fn into_reflect(self) -> Box<dyn Reflect> {
        use OwnedField::Named;
        if self.fields_spec.iter().any(|f| matches!(f, Named(_))) {
            Box::new(self.into_struct())
        } else {
            Box::new(self.into_tuple_struct())
        }
    }
}
