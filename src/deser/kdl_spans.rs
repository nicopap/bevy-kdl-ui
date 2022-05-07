//! Wrapper for cleaner access to kdl type sizes.
//!
//! God I hate this, it's going to be very inneficient most likely,
//! if every time I query for size, I have to walk through the entire
//! document and check each value sizes and add up everything :/
use std::fmt;

use super::span::Span;
use kdl::{KdlDocument, KdlEntry, KdlIdentifier, KdlNode, KdlValue};

#[derive(Clone, Copy, Debug)]
pub(super) struct NodeSizes {
    pub(super) leading: u32,
    pub(super) ty: u32,
    pub(super) name: u32,
    // includes before_children & opening curly if children exist
    pub(super) entries: u32,
    pub(super) children: u32,
    pub(super) trailing: u32,
}
impl NodeSizes {
    pub(super) fn total(&self) -> u32 {
        let NodeSizes { leading, ty, name, entries, children, trailing } = self;
        leading + ty + name + entries + children + trailing
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct DocumentSizes {
    pub(super) leading: u32,
    pub(super) nodes: u32,
    pub(super) trailing: u32,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct EntrySizes {
    pub(super) leading: u32,
    pub(super) ty: u32,
    pub(super) name: u32,
    pub(super) value: u32,
    pub(super) trailing: u32,
}
impl EntrySizes {
    pub(super) fn total(&self) -> u32 {
        let EntrySizes { leading, ty, name, value, trailing } = self;
        leading + ty + name + value + trailing
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

pub(super) trait OffsetExt {
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
            children: self.children().map_or(0, |t| t.size() + 1),
            trailing: self.trailing().size(),
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
            trailing,
        }
    }
}
impl OffsetExt for KdlDocument {
    type Out = DocumentSizes;
    fn sizes(&self) -> DocumentSizes {
        DocumentSizes {
            leading: self.leading().size(),
            nodes: self.nodes().size(),
            trailing: self.trailing().size(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SpannedEntry<'a> {
    entry: &'a KdlEntry,
    sizes: EntrySizes,
    offset: u32,
}
impl<'a> SpannedEntry<'a> {
    pub(super) fn new(entry: &'a KdlEntry, offset: u32) -> Self {
        let sizes = entry.sizes();
        Self { sizes, entry, offset }
    }
    pub(super) fn name(&self) -> Option<(Span, &'a KdlIdentifier)> {
        let EntrySizes { leading, ty, name, .. } = self.sizes;
        let name_span = Span { offset: self.offset + leading + ty, size: name };
        self.entry.name().map(|n| (name_span, n))
    }
    pub(super) fn value(&self) -> (Span, &'a KdlValue) {
        let EntrySizes { leading, ty, name, value, .. } = self.sizes;
        let value_span = Span {
            offset: self.offset + leading + ty + name,
            size: value,
        };
        (value_span, self.entry.value())
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct SpannedDocument<'a> {
    document: &'a KdlDocument,
    sizes: DocumentSizes,
    offset: u32,
}
impl<'a> SpannedDocument<'a> {
    pub(super) fn new(document: &'a KdlDocument, offset: u32) -> Self {
        let sizes = document.sizes();
        Self { sizes, document, offset }
    }
    pub(super) fn nodes(&self) -> (Span, impl Iterator<Item = SpannedNode<'a>>) {
        let offset = self.sizes.leading + self.offset;
        let nodes_span = Span { offset, size: self.sizes.nodes };
        let nodes = self.document.nodes().iter().scan(offset, |offset, n| {
            let ret = Some(SpannedNode::new(n, *offset));
            *offset += n.sizes().total();
            ret
        });
        (nodes_span, nodes)
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) struct SpannedNode<'a> {
    node: &'a KdlNode,
    sizes: NodeSizes,
    offset: u32,
}
impl<'a> SpannedNode<'a> {
    pub(super) fn new(node: &'a KdlNode, offset: u32) -> Self {
        let sizes = node.sizes();
        Self { sizes, node, offset }
    }
    pub(super) fn name(&self) -> (Span, &'a KdlIdentifier) {
        let NodeSizes { leading, ty, name, .. } = self.sizes;
        let name_span = Span { offset: self.offset + leading + ty, size: name };
        (name_span, self.node.name())
    }
    pub(super) fn entries(&self) -> (Span, impl Iterator<Item = SpannedEntry<'a>>) {
        let NodeSizes { leading, ty, name, entries, .. } = self.sizes;
        let offset = self.offset + leading + ty + name;
        let entries_span = Span { offset, size: entries };
        let entries = self.node.entries().iter().scan(offset, |offset, e| {
            let ret = Some(SpannedEntry::new(e, *offset));
            *offset += e.sizes().total();
            ret
        });
        (entries_span, entries)
    }
    pub(super) fn children(&self) -> Option<(Span, SpannedDocument<'a>)> {
        let NodeSizes { leading, ty, name, entries, children, .. } = self.sizes;
        let offset = self.offset + leading + ty + name + entries;
        let children_span = Span { offset, size: children };
        let children = self
            .node
            .children()
            .map(|n| SpannedDocument::new(n, offset));
        children.map(|c| (children_span, c))
    }
}
impl<'a> fmt::Display for SpannedNode<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.node)
    }
}
