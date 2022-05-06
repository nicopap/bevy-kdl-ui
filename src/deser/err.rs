use std::fmt::Write;

use thiserror::Error;

use super::access;
use super::span::Span;

/// Ways expectation algorithm can screw up.
#[derive(Debug, Error, PartialEq)]
pub enum ConvertError {
    #[error("This operation is unsupported: {0}")]
    GenericUnsupported(String),
    // TODO: store TypeId instead, and expected: Vec<TypeId>
    #[error("Kdl declaration has type `{actual}` but rust type `{expected}` was expected")]
    TypeMismatch { expected: String, actual: String },
    #[error("Invalid integer, value {0} out of bound for rust type: {1}")]
    IntDomain(i64, &'static str),
    #[error("Field access error: {0}")]
    Access(#[from] access::Error),
    #[error("There is no such registered type: {0}")]
    NoSuchType(String),
    #[error("Expected a value in first entry field for type: {0}, got nothing")]
    NoValuesInNode(&'static str),
    #[error("List cannot be declared using explicit positioning.")]
    NamedListDeclaration,
    #[error("Map items must be declared in the form `.field=value`.")]
    UnnamedMapDeclaration,
    #[error("Component names should never start with a dot")]
    BadComponentTypeName,
}

#[derive(Debug, PartialEq)]
pub struct ConvertErrors {
    pub(super) errors: Vec<(Span, ConvertError)>,
}
impl From<Vec<(Span, ConvertError)>> for ConvertErrors {
    fn from(errors: Vec<(Span, ConvertError)>) -> Self {
        Self { errors }
    }
}
impl ConvertErrors {
    pub fn show_for(&self, file: &str) -> String {
        let mut ret = String::with_capacity(self.errors.len() * 160);
        for (offset, error) in &self.errors {
            ret.push('\n');
            ret.push_str(file);
            writeln!(
                &mut ret,
                "\n{x: >off$}{x:->siz$}",
                off = offset.offset as usize,
                siz = offset.size as usize,
                x = ""
            )
            .unwrap();
            write!(&mut ret, "\nat {}: {error}", offset.offset).unwrap();
        }
        ret
    }
    pub fn errors(&self) -> impl Iterator<Item = &(Span, ConvertError)> {
        self.errors.iter()
    }
    pub fn show_no_context(&self) -> String {
        let mut ret = String::with_capacity(self.errors.len() * 80);
        for (offset, error) in &self.errors {
            writeln!(&mut ret, "{offset}: {error}").unwrap();
        }
        ret
    }
}
pub type ConvertResult<T> = Result<T, ConvertErrors>;
