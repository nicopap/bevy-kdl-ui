//! Wrappers around the Dynamic* of bevy_reflect to enable
//! arbitrary setting of fields.
//!
//! Since generally when defined in the KDL file, the order of field
//! declaration can vary from that of the rust declaration, it is
//! necessary to re-order the fields before pushing them into the
//! `Dynamic*` so that it is fully compatible with the corresponding
//! data type.
//!
//! (here, "rw" stands for "Random Write")
//!
//! We define 2 traits:
//! * [`RwStruct`]: A struct with fields writeable in arbitrary order.
//! * [`SeqStruct`]: A struct with fields writeable only sequentially.
//!
//! We want to exclusively use a [`RwStruct`] API, but the objects we
//! are dealing with only provide a [`SeqStruct`] API.
//!
//! We can write to fields in two mode:
//! * `Anonymous`: Values are declared without field names, the field
//!   they correspond to is implicit in the order of the values.
//! * `By field`: Values are associated with a specific field, that may
//!   be out of order from the declaration sequence.
//!
//! Anonymous write mode is _always_ sequential. By wrapping a `Dynamic*`
//! into a struct that prevents arbitrary access, we can guarentee that
//! the writes are always in the right order. This is what [`Anon`] does.
//!
//! By-field writes are super hairy. Since bevy_reflect doesn't define
//! an arbitrary access API for the `Dynamic*` structs, we are stuck
//! creating a buffer, reordering it and writting one after the other
//! the value to the `Dynamic*` thing. This is was [`Rw`] does.
//!
//! Both [`Anon`] and [`Rw`] wrap [`SeqStruct`] to provide a [`RwStruct`]
//! API.
//!
//! We also take advantage of a clear API definition to provide meaningfull
//! errors when accessing and adding fields to `Dynamic*` stuff.
use std::fmt;
use std::marker::PhantomData;

use super::access::{self, Position};
use super::DynRefl;
use bevy_reflect::{
    DynamicList, DynamicMap, DynamicStruct, DynamicTuple, DynamicTupleStruct, NamedField,
    TypeIdentity, UnnamedField,
};

/// Wraps a Dynamic* to set the fields in arbitrary order while
/// preserving the final order.
///
/// "Rw" stands for "Random Write".
pub(super) struct Rw<'r, T, F: ?Sized, M> {
    buffer: Vec<(Position, Box<F>, DynRefl)>,
    map: &'r [M],
    name: String,
    _ty: PhantomData<T>,
}
impl<'r, T, F: ?Sized, M> Rw<'r, T, F, M>
where
    T: SeqStruct<Field = F, Map = [M]>,
{
    pub(super) fn new(name: String, map: &'r [M]) -> Self {
        let _ty = PhantomData;
        let buffer = Vec::new();
        Rw { buffer, name, map, _ty }
    }
}
impl<'r, T, F: ?Sized + fmt::Debug, M> RwStruct for Rw<'r, T, F, M>
where
    T: SeqStruct<Field = F, Map = [M]>,
{
    type Out = T;
    type Field = Box<F>;
    fn add_field<T2R>(&mut self, field: Box<F>, make_value: T2R) -> Result<(), access::Error>
    where
        T2R: FnOnce(&TypeIdentity) -> Option<DynRefl>,
    {
        let oob = || access::Error::OutOfBound(format!("{field:?}"));
        let (pos, ty_id) = T::map(self.map, &field).ok_or_else(oob)?;
        if self.buffer.iter().any(|(p, _, _)| p == &pos) {
            return Err(access::Error::AlreadyExists(pos.to_string()));
        }
        if let Some(value) = make_value(ty_id) {
            self.buffer.push((pos, field, value));
        }
        Ok(())
    }
    fn complete(self) -> Result<T, access::Error> {
        if self.buffer.len() == self.map.len() {
            let Self { name, mut buffer, .. } = self;
            let mut ret = T::default();
            ret.set_name(name);
            buffer.sort_unstable_by_key(|p| p.0);
            for (_, field, value) in buffer.into_iter() {
                ret.add(&field, value);
            }
            Ok(ret)
        } else {
            Err(access::Error::FieldCountMismatch {
                expected: self.map.len(),
                actual: self.buffer.len(),
            })
        }
    }
}

/// A wrapper to force sequential anonymous access to Dynamic*.
pub(super) struct Anon<'r, T, F: ?Sized, M> {
    inner: T,
    _f: PhantomData<F>,
    map: &'r [M],
    current: u8,
}
impl<'r, T, F: ?Sized, M> Anon<'r, T, F, M>
where
    T: SeqStruct<Field = F, Map = [M]>,
{
    pub(super) fn new(name: String, map: &'r [M]) -> Self {
        let mut inner = T::default();
        inner.set_name(name);
        let _f = PhantomData;
        Self { inner, map, current: 0, _f }
    }
}
impl<'r, T, F: ?Sized, M> RwStruct for Anon<'r, T, F, M>
where
    T: SeqStruct<Field = F, Map = [M]>,
{
    type Field = ();
    type Out = T;

    /// Add directly a new anonymous field to the wrapped `DynamicFoo`
    ///
    /// Note that if `make_value` fails, the value is not added but counted
    /// "as if" it was, when doing the final field count check in [`Self::complete`].
    ///
    /// It is the responsibility of the user that if `make_value` returns a None, a
    /// corresponding error is registered in the calling code.
    fn add_field<T2R>(&mut self, (): Self::Field, make_value: T2R) -> Result<(), access::Error>
    where
        T2R: FnOnce(&TypeIdentity) -> Option<DynRefl>,
    {
        let idx = Position::new_u8(self.current);
        let oob = || access::Error::OutOfBound(format!("field .{idx}"));
        let (field, ty_id) = T::unmap(self.map, idx).ok_or_else(oob)?;
        self.current += 1;
        if let Some(value) = make_value(ty_id) {
            self.inner.add(&field, value);
        };
        Ok(())
    }
    fn complete(self) -> Result<Self::Out, access::Error> {
        if self.current as usize == self.map.len() {
            Ok(self.inner)
        } else {
            Err(access::Error::FieldCountMismatch {
                expected: self.map.len(),
                actual: self.current as usize,
            })
        }
    }
}

type Tid = TypeIdentity;

/// A thing that can be built in arbitrary order
pub(super) trait RwStruct {
    type Field;
    type Out;
    fn add_field<T2R>(&mut self, field: Self::Field, make_value: T2R) -> Result<(), access::Error>
    where
        T2R: FnOnce(&TypeIdentity) -> Option<DynRefl>;
    fn complete(self) -> Result<Self::Out, access::Error>;
}
/// A DynamicList with guarenteed type (mostly to be able to impl RwStruct on it).
///
/// Trying to add something of the wrong type to it will result in an error.
pub(super) struct HomoList {
    list: DynamicList,
    ty: TypeIdentity,
}
impl HomoList {
    // TODO(CLEAN): accept ListInfo as input, so that can do the
    // fancy things in there
    pub(super) fn new(name: String, ty: TypeIdentity) -> Self {
        let mut list = DynamicList::default();
        list.set_name(name);
        Self { list, ty }
    }
}
impl RwStruct for HomoList {
    type Field = ();
    type Out = DynamicList;
    fn add_field<T2R>(&mut self, _field: Self::Field, make_value: T2R) -> Result<(), access::Error>
    where
        T2R: FnOnce(&TypeIdentity) -> Option<DynRefl>,
    {
        if let Some(value) = make_value(&self.ty) {
            self.list.push_box(value);
            Ok(())
        } else {
            Err(access::Error::NonHomoType)
        }
    }
    fn complete(self) -> Result<Self::Out, access::Error> {
        Ok(self.list)
    }
}
/// A DynamicMap with forced string keys and homogenous values.
///
/// This is mostly in order to fit a round peg in a square hole: I have this `RwStruct` and I'd
/// rather write a mountain of code for each `DynamicFoo` that exists, so I proxy everything through
/// `RwStruct` and do not bother to increase the API surface. The code is already massive for what
/// little it does.
pub(super) struct HomoMap {
    map: DynamicMap,
    ty: TypeIdentity,
}
impl HomoMap {
    // TODO(CLEAN): accept MapInfo as input, so that can do the
    // fancy things in there, less error-prone
    pub(super) fn new(name: String, ty: TypeIdentity) -> Self {
        let mut map = DynamicMap::default();
        map.set_name(name);
        Self { map, ty }
    }
}
impl RwStruct for HomoMap {
    type Field = Box<str>;
    type Out = DynamicMap;
    fn add_field<T2R>(&mut self, field: Self::Field, make_value: T2R) -> Result<(), access::Error>
    where
        T2R: FnOnce(&TypeIdentity) -> Option<DynRefl>,
    {
        if let Some(value) = make_value(&self.ty) {
            let string_field = field.into_string();
            self.map.insert_boxed(Box::new(string_field), value);
            Ok(())
        } else {
            Err(access::Error::NonHomoType)
        }
    }
    fn complete(self) -> Result<Self::Out, access::Error> {
        Ok(self.map)
    }
}

/// A thing that can only be built sequentially.
pub(super) trait SeqStruct: Default {
    type Field: ?Sized;
    type Map: ?Sized;
    fn add(&mut self, field: &Self::Field, value: DynRefl);
    fn set_name(&mut self, name: String);
    fn map<'a>(mapping: &'a Self::Map, field: &Self::Field)
        -> Option<(Position, &'a TypeIdentity)>;
    fn unmap(mapping: &Self::Map, pos: Position) -> Option<(Box<Self::Field>, &TypeIdentity)>;
}
impl SeqStruct for DynamicStruct {
    type Field = str;
    type Map = [NamedField];
    fn add(&mut self, field: &Self::Field, value: DynRefl) {
        self.insert_boxed(field, value);
    }
    fn map<'a>(mapping: &'a Self::Map, field: &Self::Field) -> Option<(Position, &'a Tid)> {
        let mut enumerated = mapping.iter().enumerate();
        let (pos, id) = enumerated.find_map(|(i, f)| (f.name() == field).then(|| (i, f.id())))?;
        Some((Position::new(pos), id))
    }
    fn unmap(mapping: &Self::Map, pos: Position) -> Option<(Box<Self::Field>, &Tid)> {
        mapping
            .get(pos.value())
            .map(|f| (f.name().clone().into(), f.id()))
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name)
    }
}
impl SeqStruct for DynamicTuple {
    type Field = Position;
    type Map = [UnnamedField];
    fn add(&mut self, _field: &Self::Field, value: DynRefl) {
        self.insert_boxed(value);
    }
    fn unmap(mapping: &Self::Map, pos: Position) -> Option<(Box<Self::Field>, &Tid)> {
        mapping
            .get(pos.value())
            .map(move |f| (Box::new(pos), f.id()))
    }
    fn map<'a>(mapping: &'a Self::Map, field: &Self::Field) -> Option<(Position, &'a Tid)> {
        let ty_info = mapping.get(field.value())?;
        Some((*field, ty_info.id()))
    }
    fn set_name(&mut self, _name: String) {
        // Intentionally does nothing (tuples are anonymous)
    }
}
impl SeqStruct for DynamicTupleStruct {
    type Field = Position;
    type Map = [UnnamedField];
    fn add(&mut self, _field: &Self::Field, value: DynRefl) {
        self.insert_boxed(value);
    }
    fn unmap(mapping: &Self::Map, pos: Position) -> Option<(Box<Self::Field>, &Tid)> {
        mapping.get(pos.value()).map(|f| (Box::new(pos), f.id()))
    }
    fn map<'a>(mapping: &'a Self::Map, field: &Self::Field) -> Option<(Position, &'a Tid)> {
        let ty_info = mapping.get(field.value())?;
        Some((*field, ty_info.id()))
    }
    fn set_name(&mut self, name: String) {
        self.set_name(name)
    }
}
