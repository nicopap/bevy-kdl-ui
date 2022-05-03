// enum AssetRef<T> {
//     Default,
//     Path(Arc<str>),
//     Inline(T),
// #[derive(Default)]
// struct TextStyle {
//     alignment: AssetRef<TextAlignment>,
//     color: AssetRef<Color>,
//     font: AssetRef<Handle<Font>>,
// enum LeafNode {
//     Text {
//         content: String,
//         style: TextStyle,
//     Image {
//         mode: AssetRef<ImageMode>,
//         handle: AssetRef<Handle<Image>>,
// struct UiNode {
//     children: Vec<UiNode>,


/// Deserialize a component.
///
/// The component needs to be `Reflect`.
///
/// A component is represented in kdl as a node. The node's name is the
/// type name while its "entries" (the things that follow the name in kdl
/// but preceeds the list of children) represent the component's field values.
///
/// `struct`-type components (`struct Foo { bar: usize}`) use "property" fields, while
/// `tuple`-type components (`struct Bar(usize)`) use "positional argument" entries:
///
/// ```rust
/// use bevy::prelude::*;
/// use kdl::KdlDocument;
///
/// #[derive(Component, PartialEq, Clone, Reflect)]
/// #[reflect(Component, PartialEq, Clone)]
/// struct Foo {
///     bar: usize,
///     baz: String,
/// }
/// #[derive(Component, Clone, PartialEq, Reflect)]
/// #[reflect(Component, Clone, PartialEq)]
/// struct Bar(f32);
///
/// fn str_to_component<T: Reflect + Default>(text: &str) -> Option<T> {
///     let mut registry = TypeRegistry::new();
///     registry.register_type::<Foo>();
///     registry.register_type::<Bar>();
///
///     let mut document: KdlDocument = kdl_foo.parse().ok()?;
///     // !!!!!!!!!! Here !!!!!!!!!
///     let reflected = deser_component(&registry, document.nodes_mut().pop()?)?;
///     reflected.downcast_ref().cloned()
/// }
/// fn main() -> Option<()> {
///     // for struct-type components
///     let kdl_foo = r#"Foo bar=1034 baz="hello";"#;  
///     let expected_foo = Foo {
///         bar: 1034,
///         baz: "hello".to_owned(),
///     };
///     assert_eq!(str_to_component(kdl_foo), Some(expected_foo));
///     
///     // For tuple-type components
///     let kdl_bar = r#"Bar 3.0"#;  
///     let expected_bar = Bar(3.0);
///     assert_eq!(str_to_component(kdl_bar), Some(expected_bar));
///     Some(())
/// }
/// ```


// impl UiNode {
//     fn from_cuddle(node: KdlNode, registry: &TypeRegistry) -> Result<Self, anyhow::Error> {
//         let deser = ReflectDeserializer::new(&registry);
//     }
// }
// impl Struct for DynamicKdlStruct {
//     fn field(&self, name: &str) -> Option<&dyn Reflect> {
//         let i = self.field_names.iter().position(|n| n == name)?;
//         self.fields.get(i)
//     }
//     fn field_mut(&mut self, name: &str) -> Option<&mut dyn Reflect> {
//         if let Some(index) = self.field_indices.get(name) {
//             Some(&mut *self.fields[*index])
//         } else {
//             None
//         }
//     }
//     fn field_at(&self, index: usize) -> Option<&dyn Reflect> {
//         self.fields.get(index).map(|value| &*value)
//     }
//     fn field_at_mut(&mut self, index: usize) -> Option<&mut dyn Reflect> {
//         self.fields.get_mut(index).map(|value| &mut *value)
//     }
//     fn name_at(&self, index: usize) -> Option<&str> {
//         self.field_names.get(index).map(|name| name.as_ref())
//     }
//     fn field_len(&self) -> usize {
//         self.fields.len()
//     }
//     fn iter_fields(&self) -> FieldIter {
//         FieldIter {
//             struct_val: self,
//             index: 0,
//         }
//     }
//     fn clone_dynamic(&self) -> DynamicStruct {
//         self.clone()
//     }
// }
// unsafe impl Reflect for DynamicKdlStruct {
//     fn type_name(&self) -> &str {
//         &self.name
//     }
//     fn any(&self) -> &dyn Any {
//         self
//     }
//     fn any_mut(&mut self) -> &mut dyn Any {
//         self
//     }
//     fn clone_value(&self) -> Box<dyn Reflect> {
//         Box::new(self.clone_dynamic())
//     }
//     fn reflect_ref(&self) -> ReflectRef {
//         ReflectRef::Struct(self)
//     }
//     fn reflect_mut(&mut self) -> ReflectMut {
//         ReflectMut::Struct(self)
//     }
//     fn apply(&mut self, value: &dyn Reflect) {
//         if let ReflectRef::Struct(struct_value) = value.reflect_ref() {
//             for (i, value) in struct_value.iter_fields().enumerate() {
//                 let name = struct_value.name_at(i).unwrap();
//                 if let Some(v) = self.field_mut(name) {
//                     v.apply(value);
//                 }
//             }
//         } else {
//             panic!("Attempted to apply non-struct type to struct type.");
//         }
//     }
//     fn set(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> {
//         *self = value.take()?;
//         Ok(())
//     }
//     fn reflect_hash(&self) -> Option<u64> {
//         None
//     }
//     fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
//         // TODO
//         None
//     }
//     fn serializable(&self) -> Option<Serializable> {
//         // TODO: fundamentally this is untrue
//         None
//     }
// }
// macro_rules! special_match {
//     ($self:expr =>
//         Struct($struct_b:ident) => $struct_branch:expr,
//         Any($other_b:ident) => $else_branch:expr
//     ) => {
//         match $self {
//             Self::Struct($struct_b) => $struct_branch,
//             Self::Bool($other_b) => $else_branch,
//             Self::Float($other_b) => $else_branch,
//             Self::Int($other_b) => $else_branch,
//             Self::String($other_b) => $else_branch,
//         }
//     };
// }
// unsafe impl Reflect for DynamicKdlValue {
//     fn type_name(&self) -> &str {
//         match self {
//             Self::Struct(s) => s.type_name(),
//             Self::Bool(_) => std::any::type_name::<bool>(),
//             Self::Float(_) => std::any::type_name::<f64>(),
//             Self::Int(_) => std::any::type_name::<i64>(),
//             Self::String(_) => std::any::type_name::<String>(),
//         }
//     }
//     fn any(&self) -> &dyn Any {
//         self
//     }
//     fn any_mut(&mut self) -> &mut dyn Any {
//         self
//     }
//     fn clone_value(&self) -> Box<dyn Reflect> {
//         Box::new(special_match!(self =>
//             Struct(s) => s.clone_dynamic(),
//             Any(s) => s.clone()
//         ))
//     }
//     fn reflect_ref(&self) -> ReflectRef {
//         special_match!(self =>
//             Struct(s) => s.reflect_ref(),
//             Any(s) => ReflectRef::Value(s)
//         )
//     }
//     fn reflect_mut(&mut self) -> ReflectMut {
//         special_match!(self =>
//             Struct(s) => s.reflect_mut(),
//             Any(s) => ReflectMut::Value(s)
//         )
//     }
//     fn apply(&mut self, value: &dyn Reflect) {
//         special_match!(self =>
//             Struct(s) =>  if let ReflectRef::Struct(struct_value) = value.reflect_ref() {
//                 for (i, value) in struct_value.iter_fields().enumerate() {
//                     let name = struct_value.name_at(i).unwrap();
//                     if let Some(v) = self.field_mut(name) {
//                         v.apply(value);
//                     }
//                 }
//             } else {
//                 panic!("Attempted to apply non-struct type to struct type.");
//             },
//             Any(s) => ReflectMut::Value(s)
//         )
//     }
//     fn set(&mut self, value: Box<dyn Reflect>) -> Result<(), Box<dyn Reflect>> {
//         *self = value.take()?;
//         Ok(())
//     }
//     fn reflect_hash(&self) -> Option<u64> {
//         None
//     }
//     fn reflect_partial_eq(&self, value: &dyn Reflect) -> Option<bool> {
//         // TODO
//         None
//     }
//     fn serializable(&self) -> Option<Serializable> {
//         // TODO: fundamentally this is untrue
//         None
//     }
