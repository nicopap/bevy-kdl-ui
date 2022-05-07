//! Define structs specifying access to fields in a KDL node.
use nonmax::NonMaxU8;
use std::fmt;
use thiserror::Error;

use kdl::KdlIdentifier;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
pub struct Position(NonMaxU8);
impl Position {
    pub(super) fn value(&self) -> usize {
        self.0.get() as usize
    }
    pub(super) fn new(position: usize) -> Self {
        // unwrap: Only panics if a struct has more than 256 fields
        let u8: u8 = position.try_into().unwrap();
        Self::new_u8(u8)
    }
    pub(super) fn new_u8(position: u8) -> Self {
        // unwrap: Only panics if a struct has more than 255 fields
        Self(position.try_into().unwrap())
    }
}
impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    Implicit,
    Positional(Position),
    // TODO(PERF): this would save us SO MUCH allocation if this was &str.
    // But the major issue I've never figured a clean way to deal properly
    // with it in the `RwStruct` trait.
    Named(String),
}
impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Implicit => write!(f, "unspecified field"),
            Self::Positional(p) => write!(f, "field {p}"),
            Self::Named(name) => write!(f, "field \"{name}\""),
        }
    }
}
impl Field {
    fn wrong_access<T>(self, expected: Mode) -> Result<T, Error> {
        Err(Error::WrongMode { expected, actual: self })
    }
    pub(super) fn from_ident(ident: &KdlIdentifier) -> Self {
        let name = ident.value();
        match name.strip_prefix('.').map(str::parse::<u8>) {
            None => Self::Implicit,
            Some(Ok(index)) => Self::Positional(Position::new_u8(index)),
            Some(Err(_)) => Self::Named(name.split_at(1).1.to_string()),
        }
    }
    pub(super) fn anon(self) -> Result<(), Error> {
        match self {
            Self::Implicit => Ok(()),
            Self::Positional(_) | Self::Named(_) => self.wrong_access(Mode::Anon),
        }
    }
    pub(super) fn pos(self) -> Result<Box<Position>, Error> {
        match self {
            Self::Implicit | Self::Named(_) => self.wrong_access(Mode::Pos),
            Self::Positional(n) => Ok(Box::new(n)),
        }
    }
    pub(super) fn name(self) -> Result<Box<str>, Error> {
        match self {
            Self::Implicit | Self::Positional(_) => self.wrong_access(Mode::Named),
            Self::Named(n) => Ok(n.into_boxed_str()),
        }
    }
}
/// Ways to access stuff
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Anon,
    Pos,
    Named,
}
impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anon => write!(f, "anonymous access"),
            Self::Pos => write!(f, "explicity position access"),
            Self::Named => write!(f, "by-name access"),
        }
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum Error {
    #[error("Expected a {expected}, but accessed by {actual}.")]
    WrongMode { expected: Mode, actual: Field },
    #[error("Field `{0}` is declared multiple times.")]
    AlreadyExists(String),
    #[error("{0} doesn't exist in specified struct.")]
    OutOfBound(String),
    #[error("The struct has {expected} fields, but only {actual} **valid** fields were declared.")]
    FieldCountMismatch { expected: usize, actual: usize },
    #[error("The elements of rust Lists and Maps must all be of the same type, yet more than one type found.")]
    NonHomoType,
}
impl Error {
    // TODO: add a TypeInfo argument to precise which fields are possible, etc.
    pub fn help(&self) -> Option<String> {
        use Error::*;
        use Mode::*;
        match self {
            WrongMode { expected: Anon, .. } => Some("Make all the field declarations in this struct explicit or remove the `.field` from this field".to_owned()),
            WrongMode { expected: Pos, .. } => Some("Use a named position field `.0=\"like so\"`".to_owned()),
            WrongMode { expected: Named, .. } => Some("Use a named field `.like=\"so\"`".to_owned()),
            AlreadyExists(_)|OutOfBound(_) | FieldCountMismatch {..} => None,
            NonHomoType => Some("Make sure you are not mixing multiple types in a list".to_owned()),
        }
    }
}
