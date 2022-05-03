use std::fmt;
use std::num::NonZeroU8;

use super::FieldIdentError;
use kdl::KdlIdentifier;

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
#[derive(Debug)]
pub enum OwnedField {
    Implicit,
    Positional(Position),
    Named(String),
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
