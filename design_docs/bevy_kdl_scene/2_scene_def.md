## How to define an entity hierarchy?

Problem: We need a hierarchy of things rather than individual `Component`
deserialized as `Box<dyn Reflect>`.

### Template

We could define (not sure, but let's pretend), as in bevy's `DynamiScene` our
scene just as a collection of entities, and use templates to convert a tree
into a flat structure with `Children(Vec<Entity>)` components.

### Reflected nested structure

Or we could define something like:

```rust
#[derive(Reflect)]
enum ReferBy {
  Name(String),
  Id(u32),
}
#[derive(Reflect)]
struct DeserEntity {
  refer_by: Option<ReferBy>,
  components: HashMap<TypeId, Box<dyn Reflect>>,
  children: Vec<DeserEntity>,
}
```