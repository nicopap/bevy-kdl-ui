use std::fmt::Write;
use std::ops::Range;

#[cfg(feature = "fancy-errors")]
use miette::Diagnostic;
use thiserror::Error;

use super::access;
use template_kdl::span::Span;

mod miette_compat {
    #[cfg(feature = "fancy-errors")]
    pub(super) use miette::SourceSpan;

    #[cfg(not(feature = "fancy-errors"))]
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub(super) struct SourceSpan(super::Span);
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

/// Ways for the conversion from KDL to Reflect to fail
#[derive(Debug, Error, PartialEq)]
pub enum ConvertError {
    #[error("This operation is unsupported: {0}")]
    GenericUnsupported(String),
    #[error("Templating error: {0}")]
    Template(#[from] template_kdl::err::Error),
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
    #[error("Field at component declaration site.")]
    BadComponentTypeName,
}
impl ConvertError {
    fn help(&self) -> Option<String> {
        use ConvertError::*;
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
            Access(access) => access.help(),
            NoSuchType(ty) => Some(format!("Try adding it to the type registry with `reg.register::<{ty}>()`.")),
            NoValuesInNode(ty) => Some(format!("{ty} has fields, you should specify their values.")),
            NamedListDeclaration => Some("Instead of using `.foo=bar` use `bar`.".to_owned()),
            UnnamedMapDeclaration => Some("Add a key to the values.".to_owned()),
            BadComponentTypeName => Some("You are declaring a field type, but only components are expected here.".to_owned()),

        }
    }
}

#[cfg_attr(feature = "fancy-errors", derive(Diagnostic), diagnostic())]
#[derive(Debug, PartialEq, Error)]
#[error("Failed to parse source kdl file into Reflect")]
pub struct ConvertErrors {
    #[cfg_attr(feature = "fancy-errors", source_code)]
    pub(super) source_code: String,

    #[cfg_attr(feature = "fancy-errors", related)]
    pub(super) errors: Vec<SpannedError>,
}
impl From<(String, SpannedError)> for ConvertErrors {
    fn from((source_code, error): (String, SpannedError)) -> Self {
        Self { source_code, errors: vec![error] }
    }
}
impl ConvertErrors {
    pub(super) fn new(source_code: String, errors: Vec<SpannedError>) -> Self {
        Self { source_code, errors }
    }
    pub fn show_for(&self) -> String {
        let mut ret = String::with_capacity(self.errors.len() * 160);
        for SpannedError { span, error, .. } in &self.errors {
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
            write!(&mut ret, "\nat {}: {error}", span.offset()).unwrap();
        }
        ret
    }
    pub(super) fn errors(&self) -> impl Iterator<Item = &SpannedError> {
        self.errors.iter()
    }
}

#[cfg_attr(feature = "fancy-errors", derive(Diagnostic), diagnostic())]
#[derive(Debug, PartialEq, Error)]
#[error("{error}")]
#[non_exhaustive]
pub(super) struct SpannedError {
    #[cfg_attr(feature = "fancy-errors", label)]
    span: SourceSpan,

    pub(super) error: ConvertError,

    #[cfg(feature = "fancy-errors")]
    #[help]
    help: Option<String>,
}
impl From<(Span, ConvertError)> for SpannedError {
    fn from((span, error): (Span, ConvertError)) -> Self {
        Self::new(span, error)
    }
}
impl SpannedError {
    pub(super) fn new(span: Span, error: ConvertError) -> Self {
        Self {
            span: span.pair().into(),
            #[cfg(feature = "fancy-errors")]
            help: error.help(),
            error,
        }
    }
    pub(super) fn offset(&self) -> usize {
        self.span.offset()
    }
    pub(super) fn range(&self) -> Range<usize> {
        let start = self.span.offset();
        let end = start + self.span.len();
        start..end
    }
}
pub type ConvertResult<T> = Result<T, ConvertErrors>;
pub(super) type ConvResult<T> = Result<T, ConvertError>;
