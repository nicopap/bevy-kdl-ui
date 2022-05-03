use std::any;

use bevy_reflect::{
    DynamicStruct, DynamicTupleStruct, NamedField, Reflect, StructInfo, TupleStructInfo,
    TypeIdentity, TypeInfo, TypeRegistration, TypeRegistry, UnnamedField,
};
use kdl::{KdlEntry, KdlIdentifier, KdlNode, KdlValue};
use thiserror::Error;

use super::reflect::{FieldRef, OwnedField, Position};
use super::FieldIdentError;

/// Ways expectations can be refuted and therefore parsing fail
#[derive(Debug, Error)]
pub enum InvalidError {
    #[error("The specified field `{0}` is not in the expected type {{type_name}}")]
    NotAField(OwnedField),
}

/// Ways expectation algorithm can screw up.
#[derive(Debug, Error)]
pub enum ExpError {
    #[error("Field identifier is erroneous: {0}")]
    Ident(#[from] FieldIdentError),
    #[error("Ambiguous resolution: two different fields refer to the same field")]
    Ambiguous,
    #[error("Two fields encoutered that can solve into the same thing.")]
    AlreadyMet,
    #[error("Cannot reconsiliate: {0}")]
    Invalid(#[from] InvalidError),
    #[error("An implicit value was declared after an explicit one")]
    ImplicitMix,
    #[error("There is no such registered type: {0}")]
    NoSuchType(String),
    #[error("There are more declared implicit fields than there are in its rust type")]
    TooManyImplicits,
    #[error("The field count in the KDL file ({kdl}) doesn't match the rust equivalent ({rust})")]
    FieldCountMismatch { rust: u8, kdl: u8 },
    #[error("Value type mismatch: {kdl} is not a {rust}")]
    ValueTypeMismatch { rust: &'static str, kdl: KdlValue },
    #[error("kdlvy doesn't support type {0}")]
    UnsupportedType(String),
}
pub type ExpResult<T> = Result<T, ExpError>;

// TODO(PERF,QUAL): consider splitting ExpectFields in two based on the
// kind of structures it build. We are basically merging two distinct things
// and doing all kind of checks at runtime to decide between the two. :/
/// The struct context we are navigating.
enum DynFields<'r> {
    Named(&'r StructInfo),
    Implicit(&'r [UnnamedField]),
}
impl<'r> DynFields<'r> {
    fn count(&self) -> usize {
        match self {
            Self::Implicit(fields) => fields.len(),
            Self::Named(info) => info.field_len(),
        }
    }
    fn in_bound(&self, pos: Position) -> Result<(), InvalidError> {
        match self {
            Self::Implicit(fields) if fields.len() > pos.value() => Ok(()),
            _ => Err(InvalidError::NotAField(OwnedField::Positional(pos))),
        }
    }
    fn name_pos(&self, name: &str) -> Result<Position, InvalidError> {
        let err = || InvalidError::NotAField(OwnedField::Named(name.to_string()));
        match self {
            Self::Implicit(_) => Err(err()),
            Self::Named(info) => info.index_of(name).map(Position::new).ok_or_else(err),
        }
    }
    fn ty_at(&self, pos: Position) -> Option<&TypeIdentity> {
        match self {
            Self::Named(info) => info.field_at(pos.value()).map(NamedField::id),
            Self::Implicit(fields) => fields.get(pos.value()).map(UnnamedField::id),
        }
    }
}

// TODO: rename this to "field value" or smth
#[derive(Debug)]
struct Thunk {
    id: TypeIdentity,
    value: ThunkValue,
}
#[derive(Debug)]
enum ThunkValue {
    Value(KdlValue),
    Node(KdlNode),
}
fn to_reflect_int<T: Reflect>(kdl: KdlValue, f: fn(i64) -> T) -> ExpResult<Box<dyn Reflect>> {
    let vtype_err = || ExpError::ValueTypeMismatch {
        rust: any::type_name::<T>(),
        kdl: kdl.clone(),
    };
    let value = kdl.as_i64().map(f).ok_or_else(vtype_err)?;
    Ok(Box::new(value))
}
fn to_reflect_float<T: Reflect>(kdl: KdlValue, f: fn(f64) -> T) -> ExpResult<Box<dyn Reflect>> {
    let vtype_err = || ExpError::ValueTypeMismatch {
        rust: any::type_name::<T>(),
        kdl: kdl.clone(),
    };
    let value = kdl.as_f64().map(f).ok_or_else(vtype_err)?;
    Ok(Box::new(value))
}
fn to_reflect_bool(kdl: KdlValue) -> ExpResult<Box<dyn Reflect>> {
    let vtype_err = || ExpError::ValueTypeMismatch {
        rust: any::type_name::<bool>(),
        kdl: kdl.clone(),
    };
    let value = kdl.as_bool().ok_or_else(vtype_err)?;
    Ok(Box::new(value))
}
fn to_reflect_string(kdl: KdlValue) -> ExpResult<Box<dyn Reflect>> {
    let vtype_err = || ExpError::ValueTypeMismatch {
        rust: any::type_name::<String>(),
        kdl: kdl.clone(),
    };
    let value = kdl.as_string().ok_or_else(vtype_err)?.to_string();
    Ok(Box::new(value))
}
fn to_dynamic_struct(
    fields: Vec<(FieldPos, Thunk)>,
    info: &StructInfo,
    reg: &TypeRegistry,
    name: String,
) -> ExpResult<DynamicStruct> {
    let mut ret = DynamicStruct::default();
    ret.set_name(name);
    for (i, (pos, value)) in fields.into_iter().enumerate() {
        let dyn_value = value.into_dyn(reg)?;
        let i = match pos {
            FieldPos::Implicit => i,
            FieldPos::Explicit(i) => i.value(),
        };
        let field_name = info.field_at(i).unwrap().name();
        ret.insert_boxed(field_name, dyn_value);
    }
    Ok(ret)
}
fn to_dynamic_tuple_struct(
    fields: Vec<(FieldPos, Thunk)>,
    _info: &TupleStructInfo,
    reg: &TypeRegistry,
    name: String,
) -> ExpResult<DynamicTupleStruct> {
    let mut ret = DynamicTupleStruct::default();
    ret.set_name(name);
    for (_, value) in fields.into_iter() {
        // TODO FIXME: THIS IS ERROR, fields can be declared in unordered fashion
        ret.insert_boxed(value.into_dyn(reg)?);
    }
    Ok(ret)
}
impl Thunk {
    fn into_dyn(self, reg: &TypeRegistry) -> ExpResult<Box<dyn Reflect>> {
        let info = TypeRegistration::type_info;
        let registration = reg.get(self.id.type_id());
        match (self.value, registration.map(info)) {
            (ThunkValue::Node(mut kdl), Some(TypeInfo::Struct(info))) => {
                let visitor = ExpectFields {
                    dyn_fields: DynFields::Named(info),
                    already_met: Vec::new(),
                };
                let fields = visitor.all_fields(&mut kdl)?;
                let name = registration.unwrap().name().to_string();
                Ok(Box::new(to_dynamic_struct(fields, info, reg, name)?))
            }
            (ThunkValue::Node(mut kdl), Some(TypeInfo::TupleStruct(info))) => {
                let visitor = ExpectFields {
                    dyn_fields: DynFields::Implicit(info.iter().as_slice()),
                    already_met: Vec::new(),
                };
                let fields = visitor.all_fields(&mut kdl)?;
                let name = registration.unwrap().name().to_string();
                Ok(Box::new(to_dynamic_tuple_struct(fields, info, reg, name)?))
            }
            (ThunkValue::Value(kdl), Some(TypeInfo::Value(_))) => {
                match registration.unwrap().short_name() {
                    "bool" => to_reflect_bool(kdl),
                    "f64" => to_reflect_float(kdl, |v| v as f64),
                    "f32" => to_reflect_float(kdl, |v| v as f32),
                    "i8" => to_reflect_int(kdl, |v| v as i8),
                    "i16" => to_reflect_int(kdl, |v| v as i16),
                    "i32" => to_reflect_int(kdl, |v| v as i32),
                    "i64" => to_reflect_int(kdl, |v| v as i64),
                    "i128" => to_reflect_int(kdl, |v| v as i128),
                    "isize" => to_reflect_int(kdl, |v| v as isize),
                    "u8" => to_reflect_int(kdl, |v| v as u8),
                    "u16" => to_reflect_int(kdl, |v| v as u16),
                    "u32" => to_reflect_int(kdl, |v| v as u32),
                    "u64" => to_reflect_int(kdl, |v| v as u64),
                    "u128" => to_reflect_int(kdl, |v| v as u128),
                    "usize" => to_reflect_int(kdl, |v| v as usize),
                    "String" => to_reflect_string(kdl),
                    anything => todo!("Need to handle {anything:?}"),
                }
            }
            anything => todo!("Need to handle: {anything:?}"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum FieldPos {
    Explicit(Position),
    Implicit,
}

/// The current state of expectation, including already met stuff.
struct ExpectFields<'r> {
    // TODO(QUAL): rename to "fields" or more evocative
    dyn_fields: DynFields<'r>,
    already_met: Vec<FieldPos>,
}
impl<'r> ExpectFields<'r> {
    fn new(name: &str, reg: &'r TypeRegistry) -> ExpResult<Self> {
        let err = || ExpError::NoSuchType(name.to_string());
        let info = reg
            .get_with_name(&name)
            .or_else(|| reg.get_with_short_name(&name))
            .ok_or_else(err)?;

        // let info = reg.get_with_name(name).ok_or_else(err)?;
        Ok(Self {
            dyn_fields: match info.type_info() {
                TypeInfo::Struct(info) => DynFields::Named(info),
                TypeInfo::TupleStruct(fields) => DynFields::Implicit(fields.iter().as_slice()),
                whatever => todo!("Support top level for: {whatever:?}"),
            },
            already_met: Vec::new(),
        })
    }
    /// Position of `ident` in the field it represents.
    fn resolve_ref(&self, ident: Option<&KdlIdentifier>) -> ExpResult<FieldPos> {
        let implicit = |pos| pos == &FieldPos::Implicit;
        let pos = match ident.map(FieldRef::from_ident).transpose()? {
            Some(FieldRef::Implicit) | None => {
                let only_implicits = self.already_met.iter().all(implicit);
                return only_implicits
                    .then(|| FieldPos::Implicit)
                    .ok_or(ExpError::ImplicitMix);
            }
            Some(FieldRef::Named(name)) => self.dyn_fields.name_pos(name)?,
            Some(FieldRef::Positional(pos)) => {
                let _ = self.dyn_fields.in_bound(pos)?;
                pos
            }
        };
        let same_as_current = |met| matches!(met, &FieldPos::Explicit(i) if i == pos);
        match () {
            () if self.already_met.iter().any(implicit) => Err(ExpError::ImplicitMix),
            () if self.already_met.iter().any(same_as_current) => Err(ExpError::AlreadyMet),
            () => Ok(FieldPos::Explicit(pos)),
        }
    }
    /// Parse a single entry, returning its position in the Dynamic* struct and its value.
    fn entry_reflect(&self, entry: KdlEntry) -> ExpResult<(FieldPos, Thunk)> {
        let pos = self.resolve_ref(entry.name())?;
        let ty_index = match pos {
            FieldPos::Explicit(e) => e,
            FieldPos::Implicit => Position::new(self.already_met.len()),
        };
        let err = ExpError::TooManyImplicits;
        let value = Thunk {
            id: self.dyn_fields.ty_at(ty_index).ok_or(err)?.clone(),
            value: ThunkValue::Value(entry.value().clone()),
        };
        Ok((pos, value))
    }
    /// Parse a single node, returning its position in the Dynamic* struct and its value.
    ///
    /// Note that this doesn't recursively visit nodes inside. It creates a thunk
    /// for later processing.
    fn node_reflect(&self, mut node: KdlNode) -> ExpResult<(FieldPos, Thunk)> {
        let pos = self.resolve_ref(Some(node.name()))?;
        let ty_index = match pos {
            FieldPos::Explicit(e) => {
                let ty_name = node.entries_mut().remove(0);
                node.set_name(ty_name.value().as_string().unwrap());
                e
            }
            FieldPos::Implicit => {
                // TODO: there is probably a programming error here
                // let name_str = node.name_mut().value();
                // let parsed = KdlEntry::from_str(name_str)
                // let mut entries = node.entries_mut();
                // entries.insert(0, parsed)
                Position::new(self.already_met.len())
            }
        };
        let err = ExpError::TooManyImplicits;
        let value = Thunk {
            id: self.dyn_fields.ty_at(ty_index).ok_or(err)?.clone(),
            value: ThunkValue::Node(node),
        };
        Ok((pos, value))
    }
    fn all_fields(mut self, node: &mut KdlNode) -> ExpResult<Vec<(FieldPos, Thunk)>> {
        let field_count = node.entries().len() + node.children().map_or(0, |d| d.nodes().len());
        let mut fields = Vec::with_capacity(field_count);
        let only_when_correct = |count, values: Vec<_>| {
            let value_count = values.len();
            (count == value_count)
                .then(|| values)
                .ok_or(ExpError::FieldCountMismatch {
                    rust: count as u8,
                    kdl: value_count as u8,
                })
        };
        self.already_met.reserve(field_count);
        for entry in node.entries_mut().drain(..) {
            // TODO: ideally we do not fail on first error so that it's possible
            // to show all errors in the file at the same time
            let (pos, value) = self.entry_reflect(entry)?;
            fields.push((pos, value));
            self.already_met.push(pos);
        }
        let mut nodes = match node.children_mut().take() {
            None => return only_when_correct(self.dyn_fields.count(), fields),
            Some(nodes) => nodes,
        };
        for inner in nodes.nodes_mut().drain(..) {
            let (pos, value) = self.node_reflect(inner)?;
            fields.push((pos, value));
            self.already_met.push(pos);
        }
        only_when_correct(self.dyn_fields.count(), fields)
    }
}
pub fn parse_node(node: &mut KdlNode, reg: &TypeRegistry) -> ExpResult<Box<dyn Reflect>> {
    let name = node.name().value().to_owned();
    let expect = ExpectFields::new(&name, reg)?;
    let fields = expect.all_fields(node)?;
    let err = || ExpError::NoSuchType(name.to_string());
    let registration = reg
        .get_with_name(&name)
        .or_else(|| reg.get_with_short_name(&name))
        .ok_or_else(err)?;

    Ok(match registration.type_info() {
        TypeInfo::Struct(info) => Box::new(to_dynamic_struct(fields, info, reg, name.to_owned())?),
        TypeInfo::TupleStruct(info) => {
            Box::new(to_dynamic_tuple_struct(fields, info, reg, name.to_owned())?)
        }
        whatever => todo!("Support top level for: {whatever:?}"),
    })
}
