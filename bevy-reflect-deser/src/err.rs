use std::fmt::Write;

#[cfg(feature = "fancy-errors")]
use miette::Diagnostic;

use multierr_span::Spanned;

mod miette_compat {
    #[cfg(feature = "fancy-errors")]
    pub(super) use miette::SourceSpan;

    #[cfg(not(feature = "fancy-errors"))]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub(super) struct SourceSpan(template_kdle::span::Span);
    #[cfg(not(feature = "fancy-errors"))]
    impl SourceSpan {
        pub(super) fn offset(&self) -> usize {
            self.0.offset as usize
        }
        pub(super) fn len(&self) -> usize {
            self.0.size as usize
        }
    }
    #[cfg(not(feature = "fancy-errors"))]
    impl From<(usize, usize)> for SourceSpan {
        fn from((offset, size): (usize, usize)) -> Self {
            Self(super::Span { offset: offset as u32, size: size as u32 })
        }
    }
}
use miette_compat::*;
use template_kdl::{multi_err::MultiResult, Bindings};

use crate::DynRefl;

// TODO: consider using TypeId instead of &'static str and String, and convert
// into "proper" error message at one point with the help of the registry
/// Ways for the conversion from KDL to Reflect to fail
#[cfg_attr(feature = "fancy-errors", derive(Diagnostic), diagnostic())]
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
#[error("{source}")]
pub struct Error {
    pub source: ErrorType,
    #[cfg_attr(feature = "fancy-errors", label)]
    pub span: SourceSpan,

    #[cfg(feature = "fancy-errors")]
    #[help]
    help: Option<String>,
}
impl From<template_kdl::err::Error> for Error {
    fn from(terr: template_kdl::err::Error) -> Self {
        let span = terr.span();
        Self::new(&span, terr.source.into())
    }
}
impl Error {
    pub(super) fn new(span: &impl Spanned, error: ErrorType) -> Self {
        Self {
            span: span.span().pair().into(),
            #[cfg(feature = "fancy-errors")]
            help: error.help(),
            source: error,
        }
    }
    #[cfg(test)]
    pub(super) fn offset(&self) -> usize {
        self.span.offset()
    }
    #[cfg(test)]
    pub(super) fn range(&self) -> std::ops::Range<usize> {
        let start = self.span.offset();
        let end = start + self.span.len();
        start..end
    }
}
#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum ErrorType {
    #[error("This operation is unsupported: {0}")]
    GenericUnsupported(String),
    #[error("Templating error: {0}")]
    Template(#[from] template_kdl::err::ErrorType),
    #[error("Kdl declaration has type `{actual}` but rust type `{expected}` was expected")]
    TypeMismatch {
        expected: &'static str,
        actual: String,
    },
    #[error("Invalid integer, value {0} out of bound for rust type: {1}")]
    IntDomain(i64, &'static str),
    #[error("There is no such registered type: {0}")]
    NoSuchType(String),
    #[error("Expected a value in first entry field for type: {0}, got nothing")]
    NoValuesInNode(&'static str),
    #[error("Anon tuples with unkown type had a field with unknown type")]
    UntypedTupleField,
    #[error("The field {field} is declared multiple time for struct {name}")]
    MultipleSameField { name: String, field: String },
    #[error("{requested} is not a field of {name}")]
    NoSuchStructField {
        requested: String,
        name: &'static str,
        available: Vec<(String, &'static str)>,
    },
    #[error("Maps declared with pair style should only have two fields, this one has {0} fields")]
    PairMapNotPair(u8),
    #[error("{name} has {actual} fields, but the declaration contains at least {requested}")]
    TooManyFields {
        name: &'static str,
        actual: u8,
        requested: u8,
    },
    #[error("Not all fields in {name} are declared.")]
    NotEnoughStructFields {
        missing: Vec<u8>,
        name: &'static str,
        expected: Vec<String>,
    },
    #[error("{expected} fields were expected in this tuple, but only {actual} were declared")]
    NotEnoughTupleFields { actual: u8, expected: u8 },
    #[error("List cannot be declared using explicit positioning. expected `-`, got `{0}`")]
    NamedListDeclaration(String),
    #[error("{name} requires all its field to be named, but one of them wasn't.")]
    UnnamedMapField { name: &'static str },
    #[error("The declaration of this Map started in tuple style, but this field has a name.")]
    TupleMapDeclarationMixup,
    #[error("Field at component declaration site.")]
    BadComponentTypeName,
}
impl ErrorType {
    pub(crate) fn spanned(self, span: &impl Spanned) -> Error {
        Error::new(span, self)
    }
    #[cfg(feature = "fancy-errors")]
    fn help(&self) -> Option<String> {
        use strsim::levenshtein;
        use ErrorType::*;
        let max_of = |ty: &str| -> i64 {
            match ty {
                "i8" => i8::MAX as i64,
                "i16" => i16::MAX as i64,
                "i32" => i32::MAX as i64,
                "u8" => u8::MAX as i64,
                "u16" => u16::MAX as i64,
                "u32" => u32::MAX as i64,
                _ => i64::MAX,
            }
        };
        let representable = ["i8", "i16", "i32", "u8", "u16", "u32"];
        match self {
            Template(template) => template.help(),
            GenericUnsupported(_) =>Some("This error is on the TODO list!".to_owned()),
            TypeMismatch { expected, .. } => Some(format!("You probably meant to declare a {expected}.")),
            IntDomain(i, ty) if representable.contains(ty) && *i > max_of(*ty) =>
                Some(format!("{i} is larger than {}, the largest possible {ty}, try using a larger integer type.", max_of(ty))),
            IntDomain(i, u_ty) if u_ty.starts_with('u') && i.is_negative() =>
                Some(format!("Try replacing {u_ty} by i{}, or using a positive value.", u_ty.strip_prefix('u').unwrap())),
            IntDomain(..) =>Some("Either use a larger interger type or update the value to be representable with your type.".to_owned()),
            NoSuchType(ty) => Some(format!("Try adding it to the type registry with `reg.register::<{ty}>()`.")),
            NoValuesInNode(ty) => Some(format!("{ty} has fields, you should specify their values.")),
            NamedListDeclaration(_) => Some("Instead of using `foo=bar` use `bar`.".to_owned()),
            UnnamedMapField { .. } => Some("Add a key to the values.".to_owned()),
            BadComponentTypeName => Some("You are declaring a field type, but only components are expected here.".to_owned()),

            PairMapNotPair(_) => None,
            UntypedTupleField => None,
            TupleMapDeclarationMixup => None,
            MultipleSameField { .. } => Some("Remove one of the fields".to_owned()),
            TooManyFields { .. } => Some("Remove the extraneous one".to_owned()),
            NotEnoughTupleFields {..} =>  Some("Add the missing ones".to_owned()) ,
            NotEnoughStructFields { name, expected, missing } => {
                let mut missing_fields = String::with_capacity(missing.len() * 12);
                let mut first = true;
                for missed in missing.iter().map(|i| &expected[*i as usize]) {
                    if !first {
                        missing_fields.push_str(", ")
                    }
                    missing_fields.push_str(missed);
                    first = false;
                }
                Some(format!("{name} is missing the field(s) {missing_fields}"))
            }
            NoSuchStructField { requested, name, available } => {
                let closest = available.iter().min_by_key(|(s,_)| levenshtein(requested, s));
                let closest = closest.map_or("something else".to_owned(), |s| s.0.clone());
                let mut existing = String::with_capacity(available.len() * 12);
                let mut first = true;
                for (ty, _) in available.iter() {
                    if !first {
                        existing.push_str(", ")
                    }
                    existing.push_str(ty);
                    first = false;
                }
                Some(format!("{name}'s field are {existing}. Maybe you meant {closest}?"))
            }
        }
    }
}

#[cfg_attr(feature = "fancy-errors", derive(Diagnostic), diagnostic())]
#[derive(Debug, PartialEq, thiserror::Error)]
#[error("Failed to parse source kdl file into Reflect")]
pub struct ConvertErrors {
    #[cfg_attr(feature = "fancy-errors", source_code)]
    pub(super) source_code: String,

    #[cfg_attr(feature = "fancy-errors", related)]
    pub(super) errors: Vec<Error>,
}
impl ConvertErrors {
    pub(super) fn new(source_code: String, errors: Vec<Error>) -> Self {
        Self { source_code, errors }
    }
    pub fn show_for(&self) -> String {
        let mut ret = String::with_capacity(self.errors.len() * 160);
        for Error { span, source, .. } in &self.errors {
            ret.push('\n');
            ret.push_str(&self.source_code);
            writeln!(
                &mut ret,
                "\n{x: >off$}{x:->siz$}",
                off = span.offset(),
                siz = span.len(),
                x = ""
            )
            .unwrap();
            write!(&mut ret, "\nat {}: {source}", span.offset()).unwrap();
        }
        ret
    }
    #[cfg(test)]
    pub(super) fn errors(&self) -> impl Iterator<Item = &Error> {
        self.errors.iter()
    }
}

pub enum ConvertResult {
    Deserialized(DynRefl),
    Exports(Bindings),
    Errors(ConvertErrors),
}
impl ConvertResult {
    pub(crate) fn errors(repr: impl Into<String>, errors: Vec<Error>) -> Self {
        Self::Errors(ConvertErrors::new(repr.into(), errors))
    }
}
pub(super) type ConvResult<T> = Result<T, Error>;
pub type MResult<T> = MultiResult<T, Error>;
