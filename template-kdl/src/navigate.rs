use std::ops::Deref;

use kdl::{KdlEntry, KdlValue};
use mappable_rc::Marc;
use multierr_span::{Smarc, Span, Spanned};

use crate::{
    span::{SpannedIdent, SpannedNode},
    template::{Context, NodeThunk},
};

fn into<T, U: Into<T>>(from: U) -> T {
    from.into()
}

#[derive(Clone, Debug)]
pub struct Sstring {
    pub inner: Marc<str>,
    pub span: Span,
}
impl Deref for Sstring {
    type Target = str;
    fn deref(&self) -> &str {
        &*self.inner
    }
}
impl From<SpannedIdent> for Sstring {
    fn from(ident: SpannedIdent) -> Self {
        Self {
            inner: Marc::map(ident.inner.clone(), |t| t.value()),
            span: ident.span(),
        }
    }
}
impl Spanned for Sstring {
    fn span(&self) -> Span {
        self.span
    }
}
impl PartialEq for Sstring {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

pub struct ThunkField(pub(crate) ThunkField_);
impl ThunkField {
    fn node(inner: NodeThunk) -> Self {
        Self(ThunkField_::Node(inner))
    }
    fn entry(inner: Smarc<KdlEntry>, ctx: Context) -> Self {
        Self(ThunkField_::Entry(inner, ctx))
    }
}
pub(crate) enum ThunkField_ {
    Node(NodeThunk),
    Entry(Smarc<KdlEntry>, Context),
}
pub enum Value<Lst, Br> {
    List(Lst),
    Bare(Br),
}
/// A data structure that can be navigated to build a data structure out of it
pub trait Navigable<V, N> {
    type Field: Navigable<V, N> + ?Sized;
    type Fields: Iterator<Item = Self::Field>;

    fn value(&self) -> Value<Self::Fields, V>;
    fn name(&self) -> Option<N>;
    fn ty(&self) -> Option<N>;
    fn value_count(&self) -> Value<u32, ()> {
        match self.value() {
            Value::Bare(_) => Value::Bare(()),
            Value::List(lst) => Value::List(lst.count() as u32),
        }
    }
}
impl Navigable<Smarc<KdlValue>, Sstring> for ThunkField {
    type Field = ThunkField;
    type Fields = Box<dyn Iterator<Item = Self::Field>>;

    fn value(&self) -> Value<Self::Fields, Smarc<KdlValue>> {
        match &self.0 {
            ThunkField_::Node(n) => n.value(),
            ThunkField_::Entry(entry, ctx) => {
                let value = entry.value();
                let expanded = ctx.arguments.value(&*value).cloned();
                Value::Bare(expanded.unwrap_or(value))
            }
        }
    }
    fn value_count(&self) -> Value<u32, ()> {
        match &self.0 {
            ThunkField_::Node(n) => n.value_count(),
            ThunkField_::Entry(..) => Value::Bare(()),
        }
    }
    fn name(&self) -> Option<Sstring> {
        match &self.0 {
            ThunkField_::Entry(entry, _) => entry.name().map(into),
            ThunkField_::Node(node) => Navigable::name(node),
        }
    }
    fn ty(&self) -> Option<Sstring> {
        match &self.0 {
            ThunkField_::Entry(entry, _) => entry.ty().map(into),
            ThunkField_::Node(node) => node.ty(),
        }
    }
}
impl Navigable<Smarc<KdlValue>, Sstring> for NodeThunk {
    type Field = ThunkField;
    type Fields = Box<dyn Iterator<Item = Self::Field>>;

    // NOTE: if single argument child without name, then we assume it's a value.
    #[allow(unused_parens)]
    fn value(&self) -> Value<Self::Fields, Smarc<KdlValue>> {
        if self.is_value() {
            let entry = self.body.entries().next().unwrap();
            ThunkField::entry(entry, self.context.clone()).value()
        } else {
            let ctx = self.context.clone();
            let thunk_entry = move |e| ThunkField::entry(e, ctx.clone());
            let entries = self.body.entries().map(thunk_entry);
            // TODO(PERF): find something slightly more efficient than comparing every node
            // name every encountered with all bindings.
            let ctx = self.context.clone();
            let with_param_expanded = move |body| {
                let body = NodeThunk { body, context: ctx.clone() };
                let replacement = ctx.expand(&body);
                let no_repl = replacement.is_empty();
                (if no_repl { vec![body] } else { replacement })
            };
            let doc = self.body.children().into_iter();
            let children = doc
                .flat_map(|d| d.nodes())
                .flat_map(with_param_expanded)
                .map(ThunkField::node);
            Value::List(Box::new(entries.chain(children)))
        }
    }
    fn value_count(&self) -> Value<u32, ()> {
        if self.is_value() {
            Value::Bare(())
        } else {
            let entries = self.body.inner.entries().len() as u32;
            let children = self.body.inner.children().map_or(0, |c| c.nodes().len()) as u32;
            Value::List(entries + children)
        }
    }
    fn name(&self) -> Option<Sstring> {
        let name = self.body.name();
        (name.value() != "-").then(|| name.into())
    }
    // NOTE: due to `value` handling of single arg child, we should forward the arg's
    // type when we forward the arg's value.
    fn ty(&self) -> Option<Sstring> {
        if self.is_value() {
            // NOTE: it's currently impossible to change the type of entries through
            // templating, we are relying on that for this to work.
            let entry = self.body.entries().next().unwrap();
            entry.ty().map(into)
        } else {
            let name = || Navigable::name(self);
            let ty = self.body.ty();
            ty.map(into).or_else(name)
        }
    }
}
pub enum SpannedField {
    Node(SpannedNode),
    Entry(Smarc<KdlEntry>),
}
impl Navigable<Smarc<KdlValue>, Sstring> for SpannedField {
    type Field = SpannedField;
    type Fields = Box<dyn Iterator<Item = Self::Field>>;

    fn value(&self) -> Value<Self::Fields, Smarc<KdlValue>> {
        match self {
            Self::Entry(entry) => Value::Bare(entry.value()),
            Self::Node(node) => node.value(),
        }
    }
    fn value_count(&self) -> Value<u32, ()> {
        match self {
            Self::Entry(_) => Value::Bare(()),
            Self::Node(node) => node.value_count(),
        }
    }
    fn name(&self) -> Option<Sstring> {
        match self {
            Self::Entry(entry) => entry.name().map(into),
            Self::Node(node) => Some(node.name().into()),
        }
    }
    fn ty(&self) -> Option<Sstring> {
        match self {
            Self::Entry(entry) => entry.ty().map(into),
            Self::Node(node) => node.ty().map(into),
        }
    }
}
impl Navigable<Smarc<KdlValue>, Sstring> for SpannedNode {
    type Field = SpannedField;
    type Fields = Box<dyn Iterator<Item = Self::Field>>;

    // TODO: same as in NodeThunk
    fn value(&self) -> Value<Self::Fields, Smarc<KdlValue>> {
        let entries = self.entries().map(SpannedField::Entry);
        let children = self.children().into_iter().flat_map(|t| t.nodes());
        Value::List(Box::new(entries.chain(children.map(SpannedField::Node))))
    }
    fn value_count(&self) -> Value<u32, ()> {
        let entries = self.inner.entries().len() as u32;
        let children = self.inner.children().map_or(0, |c| c.nodes().len()) as u32;
        Value::List(entries + children)
    }
    fn name(&self) -> Option<Sstring> {
        Some(self.name().into())
    }
    fn ty(&self) -> Option<Sstring> {
        self.ty().map(into)
    }
}
