#[cfg(feature = "kdl-impls")]
mod kdl_impls;

use std::borrow::Borrow;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, Range};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    pub offset: u32,
    pub size: u32,
}
impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.offset, self.offset + self.size)
    }
}
impl From<Span> for Range<usize> {
    fn from(span: Span) -> Self {
        (span.offset as usize)..(span.size + span.offset) as usize
    }
}
impl Span {
    pub fn pair(&self) -> (usize, usize) {
        (self.offset as usize, self.size as usize)
    }
}

pub trait Spanned {
    fn span(&self) -> Span;
}
impl<T: Length> Spanned for (T, u32) {
    fn span(&self) -> Span {
        Span {
            size: self.0.inner_length(),
            offset: self.1 + self.0.leading(),
        }
    }
}
pub trait Length {
    fn leading(&self) -> u32 {
        0
    }
    fn trailing(&self) -> u32 {
        0
    }
    fn inner_length(&self) -> u32 {
        self.length()
    }
    /// `length` must always equal `leading + inner_length + trailing`.
    fn length(&self) -> u32;
}
impl Length for &str {
    fn length(&self) -> u32 {
        self.len() as u32
    }
}
impl<'a, T: Length> Length for &'a T {
    fn leading(&self) -> u32 {
        (*self).leading()
    }
    fn trailing(&self) -> u32 {
        (*self).trailing()
    }
    fn inner_length(&self) -> u32 {
        (*self).inner_length()
    }
    fn length(&self) -> u32 {
        (*self).length()
    }
}
impl<T: Length> Length for Option<T> {
    fn leading(&self) -> u32 {
        self.as_ref().map_or(0, |s| s.leading())
    }
    fn trailing(&self) -> u32 {
        self.as_ref().map_or(0, |s| s.trailing())
    }
    fn inner_length(&self) -> u32 {
        self.as_ref().map_or(0, |s| s.inner_length())
    }
    fn length(&self) -> u32 {
        self.as_ref().map_or(0, |s| s.length())
    }
}
impl<T: Length> Length for [T] {
    fn leading(&self) -> u32 {
        self.first().leading()
    }
    fn trailing(&self) -> u32 {
        self.last().trailing()
    }
    fn inner_length(&self) -> u32 {
        self.length() - (self.leading() + self.trailing())
    }
    fn length(&self) -> u32 {
        self.iter().map(|t| t.length()).sum()
    }
}

pub type Sref<'a, T> = Sbor<T, &'a T>;
pub type Sown<T> = Sbor<T, T>;
#[cfg(feature = "mappable-rc-impls")]
pub type Smrc<T> = Sbor<T, mappable_rc::Mrc<T>>;
#[cfg(feature = "mappable-rc-impls")]
pub type Smarc<T> = Sbor<T, mappable_rc::Marc<T>>;

// TODO: a variant with interior mutablility to memorize the
// size of itself (for example, for a deeply nested data structure,
// we potentially navigate it many times)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Sbor<T: ?Sized, B: Borrow<T>> {
    pub inner: B,
    offset: u32,
    _t: PhantomData<T>,
}
impl<T: ?Sized, B: Borrow<T>> Sbor<T, B> {
    pub fn new(inner: B, offset: u32) -> Self {
        Self { inner, offset, _t: PhantomData }
    }
    pub fn borrowed(&self) -> Sref<T> {
        Sref {
            inner: self.inner.borrow(),
            offset: self.offset,
            _t: PhantomData,
        }
    }
    pub fn map<U, C: Borrow<U>, F: FnOnce(B) -> C>(self, f: F) -> Sbor<U, C> {
        Sbor {
            inner: f(self.inner),
            offset: self.offset,
            _t: PhantomData,
        }
    }
}
impl<T: Clone, B: Borrow<T>> Sbor<T, B> {
    pub fn cloned(self) -> Sown<T> {
        Sown {
            inner: self.inner.borrow().clone(),
            offset: self.offset,
            _t: PhantomData,
        }
    }
}
impl<T: ?Sized, B: Borrow<T>> Deref for Sbor<T, B> {
    type Target = T;
    fn deref(&self) -> &T {
        self.inner.borrow()
    }
}
impl<T: ?Sized + Length, B: Borrow<T>> Spanned for Sbor<T, B> {
    fn span(&self) -> Span {
        Span {
            size: self.inner.borrow().inner_length(),
            offset: self.offset + self.inner.borrow().leading(),
        }
    }
}
