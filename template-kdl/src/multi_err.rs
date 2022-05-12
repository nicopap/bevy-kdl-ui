use crate::span::{Span, Spanned};

// TODO(perf): it is probably more efficient to use a im::Vector here instead
// of Vec.

/// Accumulates `E`s with a span.
#[derive(Debug, Clone)]
pub struct MultiError<E>(Vec<E>);
impl<E> MultiError<E> {
    pub fn into_result<T>(self, ok: T) -> MultiResult<T, E> {
        match self.0.is_empty() {
            true => MultiResult::Ok(ok),
            false => MultiResult::OkErr(ok, self.0),
        }
    }
    pub fn errors(&self) -> &[E] {
        &self.0
    }
    pub fn into_errors<T>(mut self, err: E) -> MultiResult<T, E> {
        self.0.push(err);
        MultiResult::Err(self.0)
    }
    #[doc(hidden)]
    pub fn into_many_errors<T>(mut self, errs: impl IntoIterator<Item = E>) -> MultiResult<T, E> {
        self.extend_errors(errs);
        MultiResult::Err(self.0)
    }
}
impl<E> Default for MultiError<E> {
    fn default() -> Self {
        Self(Vec::default())
    }
}
impl<E> MultiErrorTrait for MultiError<E> {
    type Error = E;

    fn add_error(&mut self, err: impl Into<Self::Error>) {
        self.0.push(err.into());
    }
    fn extend_errors(&mut self, errs: impl IntoIterator<Item = Self::Error>) {
        self.0.extend(errs);
    }
}

pub trait MultiErrorTrait {
    type Error;
    fn add_error(&mut self, err: impl Into<Self::Error>);
    fn extend_errors(&mut self, errs: impl IntoIterator<Item = Self::Error>) {
        for err in errs {
            self.add_error(err)
        }
    }
    // TODO: this shouldn't collect, should only be an adapter
    fn process_collect<I, T, C>(&mut self, iter: I) -> C
    where
        I: Iterator<Item = Result<T, Self::Error>>,
        C: FromIterator<T>,
    {
        iter.map(MultiResult::from)
            .filter_map(|t| self.optionally(t))
            .collect()
    }
    // TODO: naming
    // (Span t, t -> Res u e) -> u
    fn process<T, U, E, F>(&mut self, span: Spanned<T>, f: F) -> U
    where
        U: Default,
        Spanned<E>: Into<Self::Error>,
        F: FnOnce(T) -> Result<U, E>,
    {
        let Spanned(span, t) = span;
        match f(t) {
            Ok(ok) => ok,
            Err(err) => {
                self.add_error(Spanned(span, err));
                U::default()
            }
        }
    }
    fn optionally<R: Into<MultiResult<T, Self::Error>>, T>(&mut self, res: R) -> Option<T> {
        match res.into() {
            MultiResult::Ok(t) => Some(t),
            MultiResult::OkErr(t, errs) => {
                self.extend_errors(errs);
                Some(t)
            }
            MultiResult::Err(errs) => {
                self.extend_errors(errs);
                None
            }
        }
    }
}

pub enum MultiResult<T, E> {
    Ok(T),
    OkErr(T, Vec<E>),
    Err(Vec<E>),
}

impl<T, E> MultiResult<T, E> {
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> MultiResult<U, E> {
        match self {
            MultiResult::Ok(t) => MultiResult::Ok(f(t)),
            MultiResult::OkErr(t, errs) => MultiResult::OkErr(f(t), errs),
            MultiResult::Err(errs) => MultiResult::Err(errs),
        }
    }
    pub fn map_err<EE, F: Fn(E) -> EE>(self, f: F) -> MultiResult<T, EE> {
        match self {
            MultiResult::Ok(t) => MultiResult::Ok(t),
            MultiResult::OkErr(t, errs) => {
                let errs = errs.into_iter().map(f).collect();
                MultiResult::OkErr(t, errs)
            }
            MultiResult::Err(errs) => {
                let errs = errs.into_iter().map(f).collect();
                MultiResult::Err(errs)
            }
        }
    }
    pub fn map_err_span(self, span: Span) -> MultiResult<T, Spanned<E>> {
        self.map_err(|e| Spanned(span, e))
    }
    pub fn combine(self, errors: MultiError<E>) -> Self {
        match self {
            any_result if errors.0.is_empty() => any_result,
            MultiResult::Ok(t) => MultiResult::OkErr(t, errors.0),
            MultiResult::OkErr(t, mut errs) => {
                errs.extend(errors.0);
                MultiResult::OkErr(t, errs)
            }
            MultiResult::Err(mut errs) => {
                errs.extend(errors.0);
                MultiResult::Err(errs)
            }
        }
    }
    pub fn map_opt<U, F: FnOnce(Option<T>) -> U>(self, f: F) -> MultiResult<U, E> {
        match self {
            MultiResult::Ok(t) => MultiResult::Ok(f(Some(t))),
            MultiResult::OkErr(t, errs) => MultiResult::OkErr(f(Some(t)), errs),
            MultiResult::Err(errs) => MultiResult::OkErr(f(None), errs),
        }
    }
    pub fn unwrap_opt<U, F: FnOnce(Option<T>) -> U>(self, f: F) -> (U, Vec<E>) {
        match self {
            MultiResult::Ok(t) => (f(Some(t)), vec![]),
            MultiResult::OkErr(t, errs) => (f(Some(t)), errs),
            MultiResult::Err(errs) => (f(None), errs),
        }
    }
    pub fn into_tuple(self) -> (Option<T>, Vec<E>) {
        match self {
            MultiResult::Ok(t) => (Some(t), vec![]),
            MultiResult::OkErr(t, errs) => (Some(t), errs),
            MultiResult::Err(errs) => (None, errs),
        }
    }
    pub fn into_result(self) -> Result<T, Vec<E>> {
        use MultiResult as Mr;
        match self {
            Mr::Ok(t) => Ok(t),
            Mr::OkErr(_, errs) | Mr::Err(errs) => Err(errs),
        }
    }
}

impl<T, E> From<Result<T, E>> for MultiResult<T, E> {
    fn from(res: Result<T, E>) -> Self {
        match res {
            Ok(v) => MultiResult::Ok(v),
            Err(err) => MultiResult::Err(vec![err]),
        }
    }
}

/// Try $body. If value, then value, if no useable values, then
/// return from encompassing scope with errors accumulated in $acc
/// and the new error.
#[macro_export]
macro_rules! multi_try {
    ($acc:expr, $body:expr) => {
        match $body.into() {
            MultiResult::Ok(t) => t,
            MultiResult::OkErr(t, errs) => {
                $acc.extend_errors(errs);
                t
            }
            MultiResult::Err(errs) => return $acc.into_many_errors(errs),
        }
    };
}
