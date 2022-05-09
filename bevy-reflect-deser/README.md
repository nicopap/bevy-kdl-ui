# bevy_reflect deserialization

A deserialization format using bevy_reflect to 
get a `Box<dyn bevy_reflect::Reflect>` out of any deserializable struct.
Optionally provides great error reporting based on the [miette] library.
with the `fancy-errors` feature. You must enable it to get the nice error
messages.

Currently tightly bound to the [template-kdl] crate. But it might change
in the future.

Features:
* Parses `Box<dyn Reflect>`.
* Resilient parsing to accumulate errors rather than bailing at the first error.
* Strong typing guarentees.
* Powerfull error reporting with actionable tips, useful context and precise
  source code spans.
* Type-driven format, enables very powerful shortcuts in deserialization.

## Getting Started

Add this to your `Cargo.toml`:
```toml
bevy-reflect-deser = "0.2"
bevy_reflect = "0.8.0"
```

## Usage

`bevy-reflect-deser` Is a deserializer that returns a `Box<dyn Reflect>`.

Thanks to the excellent `Typed` API added to `bevy_reflect` in version
0.8.0, it is now possible to deserialize rust structs into a
`Box<dyn Reflect>` with extremely tight type checking. This crate provides
just that.

```kdl
Foo .name="西安" {
  // Tuples and tuple structs are anonymous
  .coordinates .1=34.265 .0=108.954
  .populations 12953000 429496 1353000
  .notable_place "Terracota army" 
}
```

For rust structures
```rust
#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct Coord(f64, f64);

#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct Foo {
    name: String,
    coordinates: Coord,
    populations: (u64, u32, u32),
    notable_place: String,
}
```

The following code will work:
```rust
use bevy_reflect::{TypeRegistry, Reflect, FromReflect};
use bevy_reflect_deser::convert_doc;

fn main() {
  let mut reg = TypeRegistry::default();
  reg.register::<String>();
  reg.register::<Coord>();
  reg.register::<f64>();
  reg.register::<u64>();
  reg.register::<u32>();
  reg.register::<(u64, u32, u32)>();
  reg.register::<Foo>();

  let doc = KDL_STRING.parse().unwrap();
  let reflected = convert_doc(&doc, &reg).unwrap();
  let foo = Foo::from_reflect(reflected.as_ref()).unwrap();
  let expected = Foo  {
    name:"西安".to_owned(),
    coordinates: Coord(108.954,34.265),
    populations: (12953000 429496 1353000),
    notable_place: "Terracota army".to_owned(),
  };
  asset_eq!(foo, expected);
}
```
TODO: add this to doctests

Check the [examples] directory for more concrete use cases.

### Special syntax

Currently (this may change in the future) if you want to refer to struct fields,
their names must be prefixed by a `.`.

## Limitations

* Currently doesn't handle at all `enum`s
* Is completely relient on [template-kdl]
* You must add all types involved to the `TypeRegistry`.

[template-kdl]: ../template-kdl/README.md
[miette]: https://crates.io/crates/miette
[examples]: ./examples
