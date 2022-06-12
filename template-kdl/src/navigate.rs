use kdl::{KdlEntry, KdlValue};
use multierr_span::Smarc;

use crate::{
    span::{SpannedIdent, SpannedNode},
    template::{Context, NodeThunk},
};

pub struct ThunkField(ThunkField_);
impl ThunkField {
    fn node(inner: NodeThunk) -> Self {
        Self(ThunkField_::Node(inner))
    }
    fn entry(inner: Smarc<KdlEntry>, ctx: Context) -> Self {
        Self(ThunkField_::Entry(inner, ctx))
    }
}
enum ThunkField_ {
    Node(NodeThunk),
    Entry(Smarc<KdlEntry>, Context),
}
pub enum Value<Lst, Br> {
    List(Lst),
    Bare(Br),
}
/// A data structure that can be navigated to build a data structure out of it
pub trait Navigable {
    type Val;
    type Field: Navigable<Val = Self::Val> + ?Sized;
    type Iter;

    fn value(&self) -> Value<Self::Iter, Self::Val>;
    fn name(&self) -> Option<SpannedIdent>;
    fn ty(&self) -> Option<SpannedIdent>;
}
impl Navigable for ThunkField {
    type Val = Smarc<KdlValue>;
    type Field = ThunkField;
    type Iter = Box<dyn Iterator<Item = Self::Field>>;

    fn value(&self) -> Value<Self::Iter, Self::Val> {
        match &self.0 {
            ThunkField_::Node(n) => n.value(),
            ThunkField_::Entry(entry, ctx) => {
                let value = entry.value();
                let expanded = ctx.arguments.value(&*value).cloned();
                Value::Bare(expanded.unwrap_or(value))
            }
        }
    }
    fn name(&self) -> Option<SpannedIdent> {
        match &self.0 {
            ThunkField_::Entry(entry, _) => entry.borrowed().name(),
            ThunkField_::Node(node) => Some(node.body.borrowed().name()),
        }
    }
    fn ty(&self) -> Option<SpannedIdent> {
        match &self.0 {
            ThunkField_::Entry(entry, _) => entry.borrowed().ty(),
            ThunkField_::Node(node) => node.body.borrowed().ty(),
        }
    }
}
impl Navigable for NodeThunk {
    type Val = Smarc<KdlValue>;
    type Field = ThunkField;
    type Iter = Box<dyn Iterator<Item = Self::Field>>;

    fn value(&self) -> Value<Self::Iter, Self::Val> {
        let single_entry = self.body.inner.entries().len() == 1;
        let first_name = self.body.inner.entries().first().map(|t| t.name());
        let no_name = first_name.map_or(false, |t| t.is_none());
        let no_children = self.body.inner.children().is_none();
        if single_entry && no_name && no_children {
            let entry = self.body.entries().next().unwrap();
            ThunkField::entry(entry, self.context.clone()).value()
        } else {
            let ctx = self.context.clone();
            let thunk_entry = move |e| ThunkField::entry(e, ctx.clone());
            let entries = self.body.entries().map(thunk_entry);
            let children = self.children().map(ThunkField::node);
            Value::List(Box::new(entries.chain(children)))
        }
    }
    fn name(&self) -> Option<SpannedIdent> {
        Some(self.name())
    }
    fn ty(&self) -> Option<SpannedIdent> {
        self.body.borrowed().ty()
    }
}
pub enum SpannedField {
    Node(SpannedNode),
    Entry(Smarc<KdlEntry>),
}
impl Navigable for SpannedField {
    type Val = Smarc<KdlValue>;
    type Field = SpannedField;
    type Iter = Box<dyn Iterator<Item = Self::Field>>;

    fn value(&self) -> Value<Self::Iter, Self::Val> {
        match self {
            Self::Entry(entry) => Value::Bare(entry.value()),
            Self::Node(node) => node.value(),
        }
    }
    fn name(&self) -> Option<SpannedIdent> {
        match self {
            Self::Entry(entry) => entry.borrowed().name(),
            Self::Node(node) => Some(node.borrowed().name()),
        }
    }
    fn ty(&self) -> Option<SpannedIdent> {
        match self {
            Self::Entry(entry) => entry.borrowed().ty(),
            Self::Node(node) => node.borrowed().ty(),
        }
    }
}
impl Navigable for SpannedNode {
    type Val = Smarc<KdlValue>;
    type Field = SpannedField;
    type Iter = Box<dyn Iterator<Item = Self::Field>>;

    fn value(&self) -> Value<Self::Iter, Self::Val> {
        let entries = self.entries().map(SpannedField::Entry);
        let children = self.children().into_iter().flat_map(|t| t.nodes());
        Value::List(Box::new(entries.chain(children.map(SpannedField::Node))))
    }
    fn name(&self) -> Option<SpannedIdent> {
        Some(self.borrowed().name())
    }
    fn ty(&self) -> Option<SpannedIdent> {
        self.borrowed().ty()
    }
}
