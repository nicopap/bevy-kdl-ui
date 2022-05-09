use std::any::{self, TypeId};
use std::fmt;
use std::string::ToString;

use crate::appendlist::AppendList;
use bevy_reflect::{
    DynamicStruct, DynamicTuple, DynamicTupleStruct, Reflect, TypeIdentity, TypeInfo,
    TypeRegistration, TypeRegistry,
};
use kdl::{KdlDocument, KdlNode, KdlValue};

use super::access::{self, Field};
use super::dyn_wrappers::{Anon, HomoList, HomoMap, Rw, RwStruct};
use super::err::{ConvResult, SpannedError};
use super::fns::{Binding, CallEntry, CallNode, Fdeclar};
use super::kdl_spans::{SpannedDocument, SpannedNode};
use super::span::Span;
use super::{ConvertError, ConvertErrors, ConvertResult, DynRefl};

pub fn convert_doc(doc: &KdlDocument, reg: &TypeRegistry) -> ConvertResult<DynRefl> {
    let doc_repr = doc.to_string();
    let spanned = SpannedDocument::new(doc, 0);
    SimpleContext::parse_document(doc_repr, spanned, reg)
}

pub fn convert_node(node: &KdlNode, registry: &TypeRegistry) -> ConvertResult<DynRefl> {
    let node_repr = node.to_string();
    let spanned = SpannedNode::new(node, 0);
    let list = AppendList::with_capacity(0);
    let ctx = Context {
        span: Span { offset: 0, size: node.len() as u32 },
        bindings: &list,
        errors: Vec::new(),
        registry,
    };
    ctx.parse_component(node_repr, spanned)
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
    fn dyn_value(self, handle: &TypeIdentity, reg: &TypeRegistry) -> ConvResult<DynRefl> {
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
    /// Converts a raw primitive type into `Box<dyn Reflect>`, making sure they have
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

/// The style of declaration for a given node. See [`super::dyn_wrapper`] module
/// level doc for details and implications. This enum is used to select how to
/// parse a given node.
#[derive(Debug, Clone, Copy)]
enum DeclarMode {
    Anon,
    ByField,
}

type FieldF<F> = fn(Field) -> Result<F, access::Error>;

/// Get type registration from `reg` with provided `name`, also tries `short_name`.
///
/// Returns `Err(NoSuchType)` if no type with provided `name` was registered.
/// Returns `Ok(None)` if `name` starts with a `.`
fn get_named<'r>(name: &str, reg: &'r TypeRegistry) -> ConvResult<Option<&'r TypeRegistration>> {
    if name.starts_with('.') {
        Ok(None)
    } else {
        reg.get_with_name(name)
            .or_else(|| reg.get_with_short_name(name))
            .map(Some)
            .ok_or(ConvertError::NoSuchType(name.to_owned()))
    }
}
struct SimpleContext<'r> {
    span: Span,
    errors: Vec<SpannedError>,
    registry: &'r TypeRegistry,
}
impl<'r> SimpleContext<'r> {
    fn parse_document<'s>(
        full_source: String,
        doc: SpannedDocument<'s>,
        registry: &'r TypeRegistry,
    ) -> ConvertResult<DynRefl> {
        let mut ctx = Self { span: doc.span(), errors: Vec::new(), registry };
        let mut nodes_remaining = doc.node_count();
        let mut nodes = doc.nodes();
        let bindings = AppendList::with_capacity(nodes_remaining - 1);
        while let Some(node) = nodes.next() {
            nodes_remaining -= 1;
            if nodes_remaining == 0 {
                return ctx
                    .with_bindings(&bindings)
                    .parse_component(full_source, node);
            }
            bindings
                .push(ctx.read_declaration(node, &bindings))
                .unwrap();
        }
        todo!("Empty KdlDocument")
    }
    fn read_span<T>(&mut self, (span, t): (Span, T)) -> T {
        self.span = span;
        t
    }
    // TODO: abstract this read_span, read_span_opt, add_error and error_resilitent
    // crap
    fn error_resilient<O, E, F>(&mut self, wrapped: F) -> Option<O>
    where
        F: FnOnce(&mut Self) -> Result<O, E>,
        E: Into<ConvertError>,
    {
        match wrapped(self) {
            Ok(v) => Some(v),
            Err(err) => self.add_error(err.into()),
        }
    }
    fn read_declaration<'s, 'i>(
        &mut self,
        node: SpannedNode<'s>,
        bindings: &'i AppendList<Binding<'s, 'i>>,
    ) -> Binding<'s, 'i> {
        let name = self.read_span(node.name());
        let declaration = self.error_resilient(|_| Fdeclar::new(node));
        Binding::new(name, bindings.as_slice(), declaration)
    }
    fn with_bindings<'s, 'i>(
        self,
        bindings: &'i AppendList<Binding<'s, 'i>>,
    ) -> Context<'r, 's, 'i> {
        let SimpleContext { span, errors, registry } = self;
        Context { span, bindings, errors, registry }
    }
    fn add_error<T>(&mut self, error: ConvertError) -> Option<T> {
        self.errors.push(SpannedError::new(self.span, error));
        None
    }
}
struct Context<'r, 's, 'i> {
    span: Span,
    bindings: &'i AppendList<Binding<'s, 'i>>,
    errors: Vec<SpannedError>,
    registry: &'r TypeRegistry,
}
impl<'r, 's, 'i> Context<'r, 's, 'i> {
    fn parse_component(
        mut self,
        full_source: String,
        node: SpannedNode<'s>,
    ) -> ConvertResult<DynRefl> {
        use ConvertError::BadComponentTypeName as BadType;
        let name = self.read_span(node.name());
        let regi = self.error_resilient(|s| get_named(name, s.registry)?.ok_or(BadType));
        let node = CallNode::new(node, self.bindings.as_slice().into());
        let dyn_for_regi = |r: &TypeRegistration| self.dyn_compound(r.type_info(), node);
        let result = regi.and_then(dyn_for_regi);
        if let Some(Some(result)) = self.errors.is_empty().then(|| result) {
            Ok(result)
        } else {
            Err(ConvertErrors::new(full_source, self.errors))
        }
    }
    fn read_span<T>(&mut self, (span, t): (Span, T)) -> T {
        self.span = span;
        t
    }
    fn read_span_opt<T>(&mut self, spanned: Option<(Span, T)>) -> Option<T> {
        if let Some((span, t)) = spanned {
            self.span = span;
            Some(t)
        } else {
            None
        }
    }
    /// Wrap a failable closure so that we  can continue walking the rest
    /// of the tree checking for other errors.
    ///
    /// We want to be able to display all errors in the file before stopping
    /// to process it.
    fn error_resilient<O, E, F>(&mut self, wrapped: F) -> Option<O>
    where
        F: FnOnce(&mut Self) -> Result<O, E>,
        E: Into<ConvertError>,
    {
        match wrapped(self) {
            Ok(v) => Some(v),
            Err(err) => self.add_error(err.into()),
        }
    }
    fn add_error<T>(&mut self, error: ConvertError) -> Option<T> {
        self.errors.push(SpannedError::new(self.span, error));
        None
    }

    fn entry2dyn<F, T>(&mut self, entry: CallEntry, acc: &mut T, get: FieldF<F>) -> ConvResult<()>
    where
        T: RwStruct<Field = F>,
    {
        use Field::Implicit;
        let field = self
            .read_span_opt(entry.name())
            .map_or(Implicit, Field::from_name);
        let make_value = move |ty_id: &TypeIdentity| self.dyn_value(ty_id, entry);
        acc.add_field(get(field)?, make_value)?;
        Ok(())
    }
    fn node2dyn<F, T>(&mut self, node: CallNode, acc: &mut T, get: FieldF<F>) -> ConvResult<()>
    where
        T: RwStruct<Field = F>,
    {
        let name = self.read_span(node.name());
        let actual = get_named(name, self.registry);
        let field = Field::from_name(name);
        let make_value = move |expected_id: &TypeIdentity| {
            let expected = self.registry.get(expected_id.type_id());
            let no_such_expected = || ConvertError::NoSuchType(expected_id.type_name().to_owned());
            let field_expected_ty = match (actual, expected) {
                (Err(err), Some(expected)) => {
                    self.add_error::<()>(err);
                    expected
                }
                (Err(err), None) => return self.add_error(err),
                (Ok(None), Some(expected)) => expected,
                (Ok(Some(actu)), Some(expect)) if actu.type_id() == expect.type_id() => expect,
                (Ok(Some(bad_actual)), Some(expected)) => {
                    self.add_error::<()>(ConvertError::TypeMismatch {
                        expected: expected.short_name().to_string(),
                        actual: bad_actual.short_name().to_string(),
                    });
                    bad_actual
                }
                (Ok(Some(bad_actual)), None) => {
                    self.add_error::<()>(no_such_expected());
                    bad_actual
                }
                (Ok(None), None) => return self.add_error(no_such_expected()),
            };
            self.dyn_compound(field_expected_ty.type_info(), node)
        };
        acc.add_field(get(field)?, make_value)?;
        Ok(())
    }
    fn read_fields_into<T, F, O>(
        &mut self,
        mut acc: T,
        node: CallNode,
        get: FieldF<F>,
    ) -> Option<DynRefl>
    where
        O: Reflect + Sized,
        T: RwStruct<Field = F, Out = O>,
    {
        for entry in self.read_span(node.entries()) {
            self.error_resilient(|s| s.entry2dyn(entry, &mut acc, get));
        }
        if let Some(doc) = node.children() {
            for inner in doc.nodes() {
                self.error_resilient(|s| s.node2dyn(inner, &mut acc, get));
            }
        }
        self.error_resilient(|_| acc.complete())
            .map(|v| Box::new(v) as DynRefl)
    }
    fn dyn_value(&mut self, expected: &TypeIdentity, entry: CallEntry) -> Option<DynRefl> {
        let value = self.read_span(entry.value());
        match KdlConcrete::from(value.clone()).dyn_value(expected, self.registry) {
            Ok(reflected) => Some(reflected),
            Err(err) => self.add_error(err),
        }
    }
    /// Build the dynamic compound value based on `node`, which should be of
    /// type `ty_info`.
    fn dyn_compound(&mut self, ty_info: &TypeInfo, node: CallNode) -> Option<DynRefl> {
        use DeclarMode::{Anon as ModAnon, ByField};
        use TypeInfo::{List, Map, Struct, Tuple, TupleStruct, Value};
        let node_name = self.read_span(node.name());
        let kdl_type = get_named(node_name, self.registry);
        let rust_type = ty_info.id();
        match kdl_type {
            Err(err) => self.add_error(err),
            Ok(Some(kdl_type)) if kdl_type.type_id() != rust_type.type_id() => {
                self.add_error(ConvertError::TypeMismatch {
                    expected: rust_type.type_name().to_string(),
                    actual: kdl_type.name().to_string(),
                })
            }
            _ => Some(()),
        };
        macro_rules! make_dyn {
            (@homogenous $accumulator:ident :: new ( $info:expr ), $getter:expr) => {{
                // TODO: this should be using something we call the macro with, but currently
                // we only call it with the i.item() and i.value() elements.
                let name = rust_type.type_name().to_string();
                self.read_fields_into($accumulator::new(name, $info.clone()), node, $getter)
            }};
            ($wrap:ident :: < $acc:ty >, $info:expr, $get:expr) => {{
                let info = $info.iter().as_slice();
                let name = $info.id().type_name().to_string();
                self.read_fields_into($wrap::<$acc, _, _>::new(name, info), node, $get)
            }};
        }
        match (self.declar_of_node(&node), ty_info) {
            (ModAnon, Tuple(i)) => make_dyn!(Anon::<DynamicTuple>, i, |_| Ok(())),
            (ByField, Tuple(i)) => make_dyn!(Rw::<DynamicTuple>, i, Field::pos),
            (ModAnon, Struct(i)) => make_dyn!(Anon::<DynamicStruct>, i, |_| Ok(())),
            (ByField, Struct(i)) => make_dyn!(Rw::<DynamicStruct>, i, Field::name),
            (ModAnon, TupleStruct(i)) => make_dyn!(Anon::<DynamicTupleStruct>, i, |_| Ok(())),
            (ByField, TupleStruct(i)) => make_dyn!(Rw::<DynamicTupleStruct>, i, Field::pos),
            (ModAnon, List(i)) => make_dyn!(@homogenous HomoList::new(i.item()), |_|Ok(())),
            (ByField, List(_)) => self.add_error(ConvertError::NamedListDeclaration),
            (ModAnon, Map(_)) => self.add_error(ConvertError::UnnamedMapDeclaration),
            (ByField, Map(i)) => make_dyn!(@homogenous HomoMap::new(i.value()), Field::name),
            (_, Value(i)) => self
                .error_resilient::<_, ConvertError, _>(|s| {
                    let err = ConvertError::NoValuesInNode(i.id().type_name());
                    let entries = s.read_span(node.entries()).next().ok_or(err)?;
                    Ok(s.dyn_value(ty_info.id(), entries))
                })
                .flatten(),
            any_else => self.add_error(ConvertError::GenericUnsupported(format!(
                "kdl node and rust type pair: {any_else:?}"
            ))),
        }
    }
    /// The style of declaration used in specified node.
    ///
    /// NOTE: if there is no fields, uses `Anon`. Empty struct (marker components)
    /// should be navigable.
    #[allow(unused_parens)]
    fn declar_of_node(&mut self, node: &CallNode) -> DeclarMode {
        use DeclarMode::{Anon, ByField};
        let ident_mode = |ident| {
            let is_anon = Field::from_name(ident).anon().is_ok();
            (if is_anon { Anon } else { ByField })
        };
        let entry = self.read_span(node.entries()).next();
        let doc = node.children();
        let first_node = doc.and_then(|d| d.nodes().next());
        entry
            .map(|e| self.read_span_opt(e.name()).map_or(Anon, ident_mode))
            .or_else(|| first_node.map(|n| ident_mode(self.read_span(n.name()))))
            .unwrap_or(Anon)
    }
}
