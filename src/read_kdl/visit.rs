use super::reflect::{DynamicKdlTypeRef, DynamicKdlValueRef, FieldRef, Position};
use super::{KdlDResult, KdlDeserError};
use kdl::{KdlDocument, KdlEntry, KdlNode};

impl<'a> TryFrom<&'a KdlDocument> for DynamicKdlTypeRef<'a> {
    type Error = KdlDeserError;
    fn try_from(doc: &'a KdlDocument) -> KdlDResult<Self> {
        use KdlDeserError::EmptyDocument;
        let last_node = doc.nodes().last().ok_or(EmptyDocument)?;
        KdlNodeProxy::from(last_node).into_struct_ref()
    }
}

fn entry_field(entry: &KdlEntry) -> KdlDResult<(FieldRef<'_>, DynamicKdlValueRef<'_>)> {
    let value = DynamicKdlValueRef::KdlValue(entry.value());
    let field = entry.name().map(FieldRef::from_ident).transpose()?;
    let field = field.unwrap_or(FieldRef::Implicit);
    Ok((field, value))
}

#[derive(Clone, Copy)]
struct KdlNodeProxy<'a> {
    name: &'a str,
    entries: &'a [KdlEntry],
    children: Option<&'a KdlDocument>,
}
impl<'a> KdlNodeProxy<'a> {
    fn as_field_value(mut self) -> KdlDResult<(FieldRef<'a>, DynamicKdlValueRef<'a>)> {
        use FieldRef::{Named, Positional};
        use KdlDeserError::InvalidFieldNode;

        let field = match self.name.strip_prefix('.').map(str::parse::<u8>) {
            None => FieldRef::Implicit,
            Some(Ok(index)) => FieldRef::Positional(Position::new_u8(index)),
            Some(Err(_)) => FieldRef::Named(self.name.split_at(1).1),
        };
        if matches!(field, Positional(_) | Named(_)) {
            self.pop_name().ok_or(InvalidFieldNode)?;
        }
        let value = self.into_struct_ref()?;
        Ok((field, DynamicKdlValueRef::Struct(value)))
    }
    fn pop_name(&mut self) -> Option<()> {
        let new_name = self.entries.get(0)?;
        let new_entries = self.entries.get(1..)?;
        self.name = new_name.value().as_string()?;
        self.entries = new_entries;
        Some(())
    }
    fn into_struct_ref(self) -> KdlDResult<DynamicKdlTypeRef<'a>> {
        let (nodes, from, as_field) = (
            KdlDocument::nodes,
            KdlNodeProxy::from,
            KdlNodeProxy::as_field_value,
        );
        let children = self.children;
        let children = children.into_iter().flat_map(nodes).map(from).map(as_field);
        let chain = self.entries.iter().map(entry_field).chain(children);
        let (fields_spec, fields) = chain.collect::<KdlDResult<Vec<_>>>()?.into_iter().unzip();
        Ok(DynamicKdlTypeRef {
            name: self.name,
            fields,
            fields_spec,
        })
    }
}
impl<'a> From<&'a KdlNode> for KdlNodeProxy<'a> {
    fn from(node: &'a KdlNode) -> Self {
        Self {
            name: node.name().value(),
            entries: node.entries(),
            children: node.children(),
        }
    }
}
