//! Wrapper for cleaner access to kdl type sizes.
//!
//! God I hate this, it's going to be very inneficient most likely,
//! if every time I query for size, I have to walk through the entire
//! document and check each value sizes and add up everything :/
use std::fmt;
use std::ops::Range;

use kdl::{KdlDocument, KdlEntry, KdlIdentifier, KdlNode, KdlValue};

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
#[derive(Clone, Copy, Debug)]
pub(crate) struct NodeSizes {
    pub(crate) leading: u32,
    pub(crate) ty: u32,
    pub(crate) name: u32,
    // includes before_children & opening curly if children exist
    pub(crate) entries: u32,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct EntrySizes {
    pub(crate) leading: u32,
    pub(crate) ty: u32,
    pub(crate) name: u32,
    pub(crate) value: u32,
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

pub(crate) trait OffsetExt {
    type Out;
    fn sizes(&self) -> Self::Out;
}
impl OffsetExt for KdlNode {
    type Out = NodeSizes;
    fn sizes(&self) -> NodeSizes {
        let opening_curly = self.children().map_or(0, |_| 1);
        let pre_children = self.before_children().size() + opening_curly;
        NodeSizes {
            leading: self.leading().size(),
            ty: self.ty().map_or(0, |t| t.size() + 2),
            name: self.name().size(),
            entries: self.entries().size() + pre_children,
        }
    }
}
impl OffsetExt for KdlEntry {
    type Out = EntrySizes;
    fn sizes(&self) -> EntrySizes {
        let leading = self.leading().size();
        // TODO: kdl-rs doesn't expose the ty of nodes
        let ty = 0; //self.ty().map_or(0, |t| t.size() + 2);
        let name = self.name().map_or(0, |t| t.size() + 1);
        let trailing = self.trailing().size();
        let remains = leading + ty + name + trailing;
        EntrySizes {
            leading,
            name,
            ty,
            // TODO: this seems hacky, workaround of kdl can have no value_repr.
            value: self
                .value_repr()
                .map_or(self.len() as u32 - remains, |t| t.size()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpannedEntry<'a> {
    entry: &'a KdlEntry,
    sizes: EntrySizes,
    offset: u32,
}
impl<'a> SpannedEntry<'a> {
    pub(crate) fn new(entry: &'a KdlEntry, offset: u32) -> Self {
        let sizes = entry.sizes();
        Self { sizes, entry, offset }
    }
    pub(crate) fn name(&self) -> Option<(Span, &'a str)> {
        let EntrySizes { leading, ty, name, .. } = self.sizes;
        let name_span = Span { offset: self.offset + leading + ty, size: name };
        self.entry.name().map(|n| (name_span, n.value()))
    }
    pub(crate) fn value(&self) -> (Span, &'a KdlValue) {
        let EntrySizes { leading, ty, name, value, .. } = self.sizes;
        let value_span = Span {
            offset: self.offset + leading + ty + name,
            size: value,
        };
        (value_span, self.entry.value())
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpannedDocument<'a> {
    document: &'a KdlDocument,
    span: Span,
    before_nodes: u32,
}
impl<'a> SpannedDocument<'a> {
    pub(crate) fn new(document: &'a KdlDocument, offset: u32) -> Self {
        let size = document.len() as u32;
        let before_nodes = document.leading().size();
        let span = Span { offset, size };
        Self { span, document, before_nodes }
    }
    pub(crate) fn nodes(&self) -> impl Iterator<Item = SpannedNode<'a>> {
        let offset = self.before_nodes + self.span.offset;
        let nodes = self.document.nodes().iter().scan(offset, |offset, n| {
            let ret = Some(SpannedNode::new(n, *offset));
            *offset += n.size();
            ret
        });
        nodes
    }
    pub(crate) fn node_count(&self) -> usize {
        self.document.nodes().len()
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpannedNode<'a> {
    node: &'a KdlNode,
    sizes: NodeSizes,
    offset: u32,
}
impl<'a> SpannedNode<'a> {
    pub(crate) fn new(node: &'a KdlNode, offset: u32) -> Self {
        let sizes = node.sizes();
        Self { sizes, node, offset }
    }
    pub(crate) fn name(&self) -> (Span, &'a str) {
        let NodeSizes { leading, ty, name, .. } = self.sizes;
        let name_span = Span { offset: self.offset + leading + ty, size: name };
        let name = self.node.name().value();
        (name_span, name)
    }
    pub(crate) fn entries(&self) -> (Span, impl Iterator<Item = SpannedEntry<'a>>) {
        let NodeSizes { leading, ty, name, entries, .. } = self.sizes;
        let offset = self.offset + leading + ty + name;
        let entries_span = Span { offset, size: entries };
        let entries = self.node.entries().iter().scan(offset, |offset, e| {
            let ret = Some(SpannedEntry::new(e, *offset));
            *offset += e.size();
            ret
        });
        (entries_span, entries)
    }
    pub(crate) fn children(&self) -> Option<SpannedDocument<'a>> {
        let NodeSizes { leading, ty, name, entries, .. } = self.sizes;
        let offset = self.offset + leading + ty + name + entries;
        self.node
            .children()
            .map(|n| SpannedDocument::new(n, offset))
    }
}
impl<'a> fmt::Display for SpannedNode<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.node)
    }
}
