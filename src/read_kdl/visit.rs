use std::any::{self, TypeId};
use std::fmt::{self, Write};
use std::mem;

use bevy_reflect::{
    DynamicStruct, DynamicTuple, DynamicTupleStruct, Reflect, TypeIdentity, TypeInfo,
    TypeRegistration, TypeRegistry,
};
use kdl::{KdlEntry, KdlNode, KdlValue};
use thiserror::Error;

use super::dyn_wrappers::{Anon, FieldError, FieldRef, HomoList, HomoMap, Rw, RwStruct};
use super::DynRefl;

/// Ways expectation algorithm can screw up.
#[derive(Debug, Error)]
pub enum ExpError {
    #[error("This operation is unsupported: {0}")]
    GenericUnsupported(&'static str),
    #[error("Kdl declaration has type `{actual}` but rust type `{expected}` was expected")]
    TypeMismatch { expected: String, actual: String },
    #[error("Invalid integer, value {0} out of bound for rust type: {1}")]
    IntError(i64, &'static str),
    #[error("Field access error: {0}")]
    FieldError(#[from] FieldError),
    #[error("There is no such registered type: {0}")]
    NoSuchType(String),
    // TODO: try remove this, seee what it does
    #[error("The type name was malformed: {0}")]
    MalformedType(String),
    #[error("Expected a value in first entry field for type: {0}, got nothing")]
    NoValuesInNode(&'static str),
    #[error("List cannot be declared using explicit positioning.")]
    NamedListDeclaration,
    #[error("Map items must be declared in the form `.field=value`.")]
    UnnamedMapDeclaration,
    #[error("Component names should never start with a dot.")]
    BadComponentTypeName,
}
type ExpResult<T> = Result<T, ExpError>;

pub fn parse_node(node: &mut KdlNode, reg: &TypeRegistry) -> ParseResult<DynRefl> {
    let my_node = mem::replace(node, KdlNode::new("foo"));
    Context::parse_component(my_node, reg)
}
/// A proxy for [`KdlValue`] that doesn't care about the format of declaration.
enum KdlConcrete {
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
    /// Try to get a Box<dyn Reflect> corresponding to provided `handle` type from this
    /// [`KdlConcrete`].
    ///
    /// Inspects recursively newtype-style structs (aka structs will a single field) if
    /// `handle` proves to be such a thing.
    ///
    /// This is useful to inline in entry position newtype struct.
    fn dyn_value(self, handle: &TypeIdentity, reg: &TypeRegistry) -> ExpResult<DynRefl> {
        self.dyn_value_newtypes(handle, reg, Vec::new())
    }
    /// Recursively resolves newtype structs attempting to summarize them into a primitive
    /// type.
    fn dyn_value_newtypes(
        self,
        handle: &TypeIdentity,
        reg: &TypeRegistry,
        mut wrappers: Vec<&'static str>,
    ) -> ExpResult<DynRefl> {
        use TypeInfo::{Struct, Tuple, TupleStruct, Value};
        wrappers.push(handle.type_name());
        let mismatch = |actual| {
            || ExpError::TypeMismatch {
                expected: if wrappers.len() == 1 {
                    format!("{}", wrappers[0])
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
            None => Err(ExpError::NoSuchType(handle.type_name().to_string())),
            Some(Struct(info)) if info.field_len() == 0 => {
                match (self, reg.get(handle.type_id())) {
                    (Self::Str(s), Some(reg)) if reg.short_name() == s || reg.name() == s => {
                        let mut ret = DynamicStruct::default();
                        ret.set_name(handle.type_name().to_string());
                        Ok(Box::new(ret))
                    }
                    (_, None) => Err(ExpError::NoSuchType(handle.type_name().to_string())),
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
    /// Converts a raw primitive type into `Box<dyn Reflect>`, making sure they have
    /// the same type as the `handle` provides.
    fn dyn_primitive_value(
        self,
        handle: &TypeIdentity,
        mismatch: impl FnOnce() -> ExpError,
    ) -> ExpResult<DynRefl> {
        use KdlConcrete::*;
        macro_rules! int2dyn {
            (@opt $int_type:ty, $int_value:expr) => {{
                Ok(Box::new(<$int_type>::try_from($int_value).ok()))
            }};
            ($int_type:ty, $int_value:expr) => {
                <$int_type>::try_from($int_value)
                    .map_err(|_| ExpError::IntError($int_value, any::type_name::<$int_type>()))
                    .map::<DynRefl, _>(|i| Box::new(i))
            };
        }
        let msg = "null values currently cannot be converted into rust types";
        let unsupported = Err(ExpError::GenericUnsupported(msg));
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

            (Null, _) => unsupported,
        }
    }
}

/// The style of declaration for a given node. See [`super::dyn_wrapper`] module
/// level doc for details and implications. This enum is used to select how to
/// parse a given node.
#[derive(Debug, Clone, Copy)]
enum DeclarMode {
    Anon,
    ByField,
}
impl DeclarMode {
    /// The style of declaration used in specified node.
    ///
    /// NOTE: if there is no fields, uses `Anon`. Empty struct (marker components)
    /// should be navigable.
    #[allow(unused_parens)]
    fn of_node(node: &KdlNode) -> DeclarMode {
        use DeclarMode::{Anon, ByField};
        let ident_mode = |ident| {
            let is_anon = FieldRef::from_ident(ident).anon().is_ok();
            (if is_anon { Anon } else { ByField })
        };
        let entry = node.entries().first();
        let inner = node.children().and_then(|doc| doc.nodes().first());
        entry
            .map(|e| e.name().map_or(Anon, ident_mode))
            .or_else(|| inner.map(|n| ident_mode(n.name())))
            .unwrap_or(Anon)
    }
}

type FieldF<F> = fn(FieldRef) -> Result<F, FieldError>;

#[derive(Debug)]
pub struct ParseErrors {
    errors: Vec<(usize, ExpError)>,
}
impl From<Vec<(usize, ExpError)>> for ParseErrors {
    fn from(errors: Vec<(usize, ExpError)>) -> Self {
        Self { errors }
    }
}
impl From<ExpError> for ParseErrors {
    fn from(error: ExpError) -> Self {
        Self {
            errors: vec![(0, error)],
        }
    }
}
impl ParseErrors {
    pub fn show_for(&self, _file: String) -> String {
        todo!()
    }
    pub fn show_no_context(&self) -> String {
        let mut ret = String::with_capacity(self.errors.len() * 80);
        for (offset, error) in &self.errors {
            writeln!(&mut ret, "{offset}: {error}").unwrap();
        }
        ret
    }
}
fn get_named<'r>(name: &str, reg: &'r TypeRegistry) -> ExpResult<Option<&'r TypeRegistration>> {
    if name.starts_with('.') {
        Ok(None)
    } else {
        reg.get_with_name(name)
            .or_else(|| reg.get_with_short_name(name))
            .map(Some)
            .ok_or(ExpError::NoSuchType(name.to_owned()))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Offset(usize);
struct Context<'r> {
    offset: usize,
    errors: Vec<(usize, ExpError)>,
    registry: &'r TypeRegistry,
}
pub type ParseResult<T> = Result<T, ParseErrors>;
impl<'r> Context<'r> {
    fn parse_component(node: KdlNode, registry: &'r TypeRegistry) -> ParseResult<DynRefl> {
        let name = node.name().value();
        let offset = node.leading().map_or(0, |s| s.len());
        let err = || vec![(offset, ExpError::BadComponentTypeName)];
        let registration = get_named(name, registry)?.ok_or_else(err)?;

        let mut ctx = Self {
            offset,
            errors: Vec::new(),
            registry,
        };
        let ty_info = registration.type_info();
        let result = ctx.parse_type(ty_info, node);
        (ctx.errors.is_empty())
            .then(|| result)
            .flatten()
            .ok_or_else(|| ctx.errors.into())
    }
    /// Wrap a failable closure so that we  can continue walking the rest
    /// of the tree checking for other errors.
    ///
    /// We want to be able to display all errors in the file before stopping
    /// to process it.
    fn error_resilient<O, E, F>(&mut self, wrapped: F) -> Option<O>
    where
        F: FnOnce(&mut Self) -> Result<O, E>,
        E: Into<ExpError>,
    {
        match wrapped(self) {
            Ok(v) => Some(v),
            Err(err) => self.add_error(0, err.into()),
        }
    }
    fn add_error<T>(&mut self, span: usize, error: ExpError) -> Option<T> {
        self.errors.push((self.offset, error));
        self.offset += span;
        None
    }
    fn advance(&mut self, span: usize) {
        self.offset += span;
    }

    fn entry2dyn<F, T>(&mut self, mut entry: KdlEntry, acc: &mut T, get: FieldF<F>) -> ExpResult<()>
    where
        T: RwStruct<Field = F>,
    {
        use FieldRef::Implicit;
        let field = entry.name().map_or(Implicit, FieldRef::from_ident);
        let make_value = move |ty_id: &TypeIdentity| self.parse_value(ty_id, &mut entry);
        acc.add_field(get(field)?, make_value)?;
        Ok(())
    }
    fn node2dyn<F, T>(&mut self, node: KdlNode, acc: &mut T, get: FieldF<F>) -> ExpResult<()>
    where
        T: RwStruct<Field = F>,
    {
        let field = FieldRef::from_ident(node.name());
        let actual = get_named(node.name().value(), self.registry);
        let make_value = move |expected_id: &TypeIdentity| {
            let expected = self.registry.get(expected_id.type_id());
            let no_such_expected = || ExpError::NoSuchType(expected_id.type_name().to_owned());
            match (actual, expected) {
                (Err(err), Some(expected)) => {
                    self.add_error::<()>(0, err);
                    self.parse_type(expected.type_info(), node)
                }
                (Err(err), None) => self.add_error(0, err),
                (Ok(None), Some(expected)) => self.parse_type(expected.type_info(), node),
                (Ok(Some(actual)), Some(expected)) if actual.type_id() == expected.type_id() => {
                    self.parse_type(expected.type_info(), node)
                }
                (Ok(Some(bad_actual)), Some(expected)) => {
                    let err = ExpError::TypeMismatch {
                        expected: expected.short_name().to_string(),
                        actual: bad_actual.short_name().to_string(),
                    };
                    self.add_error::<()>(0, err);
                    self.parse_type(bad_actual.type_info(), node)
                }
                (Ok(Some(bad_actual)), None) => {
                    self.add_error::<()>(0, no_such_expected());
                    self.parse_type(bad_actual.type_info(), node)
                }
                (Ok(None), None) => self.add_error(0, no_such_expected()),
            }
        };
        acc.add_field(get(field)?, make_value)?;
        Ok(())
    }
    fn parse_node<T, F, O>(
        &mut self,
        mut node: KdlNode,
        mut acc: T,
        get: FieldF<F>,
    ) -> Option<DynRefl>
    where
        O: Reflect + Sized,
        T: RwStruct<Field = F, Out = O>,
    {
        for entry in node.entries_mut().drain(..) {
            self.error_resilient(|s| s.entry2dyn(entry, &mut acc, get));
        }
        if let Some(mut nodes) = node.children_mut().take() {
            for inner in nodes.nodes_mut().drain(..) {
                self.error_resilient(|s| s.node2dyn(inner, &mut acc, get));
            }
        }
        self.error_resilient(|_| acc.complete())
            .map(|v| Box::new(v) as DynRefl)
    }
    fn parse_value(&mut self, expected: &TypeIdentity, entry: &mut KdlEntry) -> Option<DynRefl> {
        let value_len = entry.value_repr().map_or(0, |s| s.len());
        let value = mem::replace(entry.value_mut(), KdlValue::Null);
        match KdlConcrete::from(value).dyn_value(expected, self.registry) {
            Ok(reflected) => {
                self.advance(value_len);
                Some(reflected)
            }
            Err(err) => self.add_error(value_len, err),
        }
    }
    fn parse_type(&mut self, ty_info: &TypeInfo, mut node: KdlNode) -> Option<DynRefl> {
        use DeclarMode::{Anon as ModAnon, ByField};
        use TypeInfo::{List, Map, Struct, Tuple, TupleStruct, Value};
        let kdl_node_name = node.name().value();
        let kdl_type = get_named(kdl_node_name, self.registry);
        let rust_type = ty_info.id();
        match kdl_type {
            Err(err) => self.add_error(0, err),
            Ok(Some(kdl_type)) if kdl_type.type_id() != rust_type.type_id() => self.add_error(
                0,
                ExpError::TypeMismatch {
                    expected: rust_type.type_name().to_string(),
                    actual: kdl_type.name().to_string(),
                },
            ),
            _ => Some(()),
        };

        macro_rules! parse {
            (@homogenous $accumulator:ident :: new ( $info:expr ), $getter:expr) => {{
                // TODO: this should be using something we call the macro with, but currently
                // we only call it with the i.item() and i.value() elements.
                let name = rust_type.type_name().to_string();
                self.parse_node(node, $accumulator::new(name, $info.clone()), $getter)
            }};
            ($wrap:ident :: < $acc:ty >, $info:expr, $get:expr) => {{
                let info = $info.iter().as_slice();
                let name = $info.id().type_name().to_string();
                self.parse_node(node, $wrap::<$acc, _, _>::new(name, info), $get)
            }};
        }
        let ret = match (DeclarMode::of_node(&node), ty_info) {
            (ModAnon, Tuple(i)) => parse!(Anon::<DynamicTuple>, i, |_| Ok(())),
            (ByField, Tuple(i)) => parse!(Rw::<DynamicTuple>, i, FieldRef::pos),
            (ModAnon, Struct(i)) => parse!(Anon::<DynamicStruct>, i, |_| Ok(())),
            (ByField, Struct(i)) => parse!(Rw::<DynamicStruct>, i, FieldRef::name),
            (ModAnon, TupleStruct(i)) => parse!(Anon::<DynamicTupleStruct>, i, |_| Ok(())),
            (ByField, TupleStruct(i)) => parse!(Rw::<DynamicTupleStruct>, i, FieldRef::pos),
            (ModAnon, List(i)) => parse!(@homogenous HomoList::new(i.item()), |_|Ok(())),
            (ByField, List(_)) => self.add_error(0, ExpError::NamedListDeclaration),
            (ModAnon, Map(_)) => self.add_error(0, ExpError::UnnamedMapDeclaration),
            (ByField, Map(i)) => parse!(@homogenous HomoMap::new(i.value()), FieldRef::name),
            (_, Value(i)) => self
                .error_resilient::<_, ExpError, _>(|s| {
                    let err = ExpError::NoValuesInNode(i.id().type_name());
                    let entry = node.entries_mut().get_mut(0).ok_or(err)?;
                    Ok(s.parse_value(ty_info.id(), entry))
                })
                .flatten(),
            unsupported => todo!("implement top level parsing for: {unsupported:?}"),
        };
        ret
    }
}
