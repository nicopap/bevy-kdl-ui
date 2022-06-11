//! Wrapper for cleaner access to kdl type sizes.
//!
//! God I hate this, it's going to be very inneficient most likely,
//! if every time I query for size, I have to walk through the entire
//! document and check each value sizes and add up everything :/
use std::fmt;
use std::ops::Range;

use kdl::{KdlDocument, KdlEntry, KdlIdentifier, KdlNode, KdlValue};
use mappable_rc::Marc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    pub offset: u32,
    pub size: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Spanned<T>(pub Span, pub T);
impl<T> Spanned<T> {
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Spanned<U> {
        let Spanned(span, t) = self;
        Spanned(span, f(t))
    }
    pub fn map_err<U, E, F: FnOnce(T) -> Result<U, E>>(self, f: F) -> Result<U, Spanned<E>> {
        let Self(span, t) = self;
        f(t).map_err(|u| Spanned(span, u))
    }
}
impl<'a, T: Clone> Spanned<&'a T> {
    pub fn cloned(self) -> Spanned<T> {
        self.map(|t| t.clone())
    }
    pub fn map_cloned<U, F: FnOnce(T) -> U>(self, f: F) -> Spanned<U> {
        let Spanned(span, t) = self;
        Spanned(span, f(t.clone()))
    }
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
trait AdhocLen {
    fn size(&self) -> u32;
}
impl<T: AdhocLen> AdhocLen for Option<T> {
    fn size(&self) -> u32 {
        self.as_ref().map_or(0, |s| s.size())
    }
}
impl<'a> AdhocLen for &'a str {
    fn size(&self) -> u32 {
        self.len() as u32
    }
}
impl<'a, T: AdhocLen> AdhocLen for &'a [T] {
    fn size(&self) -> u32 {
        self.iter().map(|t| t.size()).sum()
    }
}
impl AdhocLen for KdlIdentifier {
    fn size(&self) -> u32 {
        self.repr().map_or(self.value().len() as u32, |s| s.size())
    }
}
impl AdhocLen for KdlEntry {
    fn size(&self) -> u32 {
        self.len() as u32
    }
}
impl AdhocLen for KdlDocument {
    fn size(&self) -> u32 {
        self.len() as u32
    }
}
impl AdhocLen for KdlNode {
    fn size(&self) -> u32 {
        self.len() as u32
    }
}
impl AdhocLen for KdlValue {
    fn size(&self) -> u32 {
        let must_escape = ['\n', '\\', '"', '\r', '\t', '\u{08}', '\u{0C}'];
        match self {
            KdlValue::Base10Float(value) => {
                let clean = match () {
                    () if value == &f64::INFINITY => f64::MAX,
                    () if value == &f64::NEG_INFINITY => -f64::MAX,
                    () if value.is_nan() => 0.0,
                    () => *value,
                };
                format!("{clean:?}").len() as u32
            }
            KdlValue::Bool(true) => 4,
            KdlValue::Bool(false) => 5,
            KdlValue::Null => 4,
            KdlValue::RawString(_) => format!("{self}").len() as u32,
            KdlValue::String(s) => (s.len() + 2 + s.matches(must_escape).count()) as u32,
            KdlValue::Base2(value) => 2 + (64 - value.leading_zeros()),
            KdlValue::Base8(value) => 2 + ((64 - value.leading_zeros()) / 3),
            KdlValue::Base16(value) => 2 + ((64 - value.leading_zeros()) / 4),
            KdlValue::Base10(value) => format!("{value:?}").len() as u32,
        }
    }
}

trait KdlNodeSizeExt {
    fn ty_len(&self) -> u32;
    fn name_len(&self) -> u32;
    fn entries_len(&self) -> u32;
    fn children_len(&self) -> u32;
    fn full_len(&self) -> u32;
}
impl KdlNodeSizeExt for KdlNode {
    fn ty_len(&self) -> u32 {
        self.ty().map_or(0, |t| t.size() + 2)
    }
    fn name_len(&self) -> u32 {
        self.name().size()
    }
    fn entries_len(&self) -> u32 {
        let opening_curly = self.children().map_or(0, |_| 1);
        let pre_children = self.before_children().size() + opening_curly;
        self.entries().size() + pre_children
    }
    fn children_len(&self) -> u32 {
        self.children().map_or(0, |c| c.size())
    }
    fn full_len(&self) -> u32 {
        self.ty_len() + self.name_len() + self.entries_len() + self.children_len()
    }
}
trait KdlEntrySizeExt {
    fn leading_len(&self) -> u32;
    fn ty_len(&self) -> u32;
    fn name_len(&self) -> u32;
    fn value_len(&self) -> u32;
}
impl KdlEntrySizeExt for KdlEntry {
    fn leading_len(&self) -> u32 {
        self.leading().size()
    }
    fn ty_len(&self) -> u32 {
        self.ty().map_or(0, |t| t.size() + 2)
    }
    fn name_len(&self) -> u32 {
        self.name().map_or(0, |t| t.size() + 1)
    }
    fn value_len(&self) -> u32 {
        match self.value_repr() {
            Some(repr) => repr.size(),
            None => self.value().size(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SpannedIdent<'a> {
    ident: &'a KdlIdentifier,
    offset: u32,
}
impl<'a> SpannedIdent<'a> {
    pub fn value(self) -> &'a str {
        self.ident.value()
    }
    pub fn span(self) -> Span {
        Span { offset: self.offset, size: self.ident.size() }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpannedEntry<'a> {
    pub(crate) entry: &'a KdlEntry,
    offset: u32,
}
impl<'a> SpannedEntry<'a> {
    pub(crate) fn new(entry: &'a KdlEntry, offset: u32) -> Self {
        Self { entry, offset }
    }
    pub(crate) fn name(&self) -> Option<SpannedIdent<'a>> {
        let leading = self.entry.leading_len();
        let ident = self.entry.name()?;
        Some(SpannedIdent { offset: self.offset + leading, ident })
    }
    pub(crate) fn value(&self) -> Spanned<&'a KdlValue> {
        let leading = self.entry.leading_len();
        let ty = self.entry.ty_len();
        let name = self.entry.name_len();
        let value_span = Span {
            offset: self.offset + leading + ty + name,
            size: self.entry.value_len(),
        };
        Spanned(value_span, self.entry.value())
    }
    pub(crate) fn ty(&self) -> Option<SpannedIdent<'a>> {
        let leading = self.entry.leading_len();
        let name = self.entry.name_len();
        let ident = self.entry.ty()?;
        Some(SpannedIdent { offset: self.offset + leading + name + 1, ident })
    }
    pub(crate) fn span(&self) -> Span {
        Span { offset: self.offset, size: self.entry.size() }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SpannedDocument {
    doc: Marc<[KdlNode]>,
    offset: u32,
}
impl SpannedDocument {
    pub(crate) fn new(mut doc: KdlDocument) -> Self {
        let nodes = std::mem::take(doc.nodes_mut());
        Self { offset: doc.leading().size(), doc: nodes.into() }
    }
    pub(crate) fn node_count(&self) -> usize {
        self.doc.len()
    }
    pub(crate) fn nodes(&self) -> impl Iterator<Item = SpannedNode> {
        let mut i = 0;
        let mut offset = self.offset;
        let doc = self.doc.clone();
        std::iter::from_fn(move || {
            let node = Marc::try_map(doc.clone(), |nodes| nodes.get(i)).ok()?;
            let new_offset = offset + node.leading().size() + node.full_len();
            let result = SpannedNode { node, offset };
            offset = new_offset;
            i += 1;
            Some(result)
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SpannedNode {
    node: Marc<KdlNode>,
    offset: u32,
}
impl SpannedNode {
    pub(crate) fn ty(&self) -> Option<SpannedIdent> {
        Some(SpannedIdent { offset: self.offset + 1, ident: self.node.ty()? })
    }
    pub(crate) fn name(&self) -> SpannedIdent {
        SpannedIdent {
            offset: self.offset + self.node.ty_len(),
            ident: self.node.name(),
        }
    }
    pub(crate) fn entries(&self) -> impl Iterator<Item = SpannedEntry> {
        let ty = self.node.ty_len();
        let name = self.node.name_len();
        let offset = self.offset + ty + name;
        let entries = self.node.entries().iter().scan(offset, |offset, e| {
            let cur_offset = *offset;
            *offset += e.size();
            Some(SpannedEntry::new(e, cur_offset))
        });
        entries
    }
    pub(crate) fn children(&self) -> Option<SpannedDocument> {
        let ty = self.node.ty_len();
        let name = self.node.name_len();
        let entries = self.node.entries_len();
        let offset = self.offset + ty + name + entries;
        Marc::try_map(self.node.clone(), |n| n.children())
            .ok()
            .map(|doc| SpannedDocument {
                offset: offset + doc.leading().size(),
                doc: Marc::map(doc, |d| d.nodes()),
            })
    }
    pub(crate) fn fields(&self) -> impl Iterator<Item = (Option<&str>, Option<&KdlValue>)> {
        let entries = self
            .entries()
            .map(|e| (e.name().map(|e| e.value()), Some(e.value().1)));
        let children = self.node.children().into_iter();
        let nodes = children.flat_map(|c| c.nodes()).map(|n| {
            let value = n.entries().first().map(|v| v.value());
            (Some(n.name().value()), value)
        });
        entries.chain(nodes)
    }
    pub(crate) fn span(&self) -> Span {
        let ty = self.node.ty_len();
        let name = self.node.name_len();
        let entries = self.node.entries_len();
        let children = self.node.children_len();
        Span {
            offset: self.offset,
            size: ty + name + entries + children,
        }
    }
}
impl<'a> fmt::Display for SpannedNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.node)
    }
}
