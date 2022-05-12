# bevy_reflect for kdl

A deserialization format using bevy_reflect to 
get a `Box<dyn bevy_reflect::Reflect>` out of any deserializable struct.
Optionally provides great error reporting based on the [miette] library.
with the `fancy-errors` feature. It is enabled by default.

**Only works with [template-kdl] crate**. It is likely to not change
because KDL doesn't translate well to the serde data format.

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
bevy-reflect-deser = "0.3.0"
bevy_reflect = "0.8.0"
```

If you do not want the `fancy-errors` feature, you should disable it this way:

```toml
bevy-reflect-deser = { version = "0.3.0", no-default-features = true }
```

## Limitations

* Currently doesn't handle at all `enum`s
* Only works with [template-kdl]
* You must add all types involved to the `TypeRegistry`.

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

Check the [examples] directory for more concrete use cases.

## Kdl jargon

The following kdl code is a *node* with the *node name* `NodeName` and two
*entries*. The first *entry* is a *parameter* because it is a key/value pair.
The *parameter name* is `parameter_name`, and the parmater's *value* is
`parameter_value`. The second entry is an *argument* because it is only a value,
not a key/value pair. The *argument* has as value, the string `argument`.

Therefore, the *node* `NodeName` has two *entry values*: `parameter_value` and
`argument`.

A node can have *children*. The children of the node is a series of nodes.

```kdl
NodeName parameter_name="parameter_value" "argument" {
  OtherNode
  MoreNode "with" "argument"
}
```

## Declarations

There are two types of declarations in `bevy-reflect-deser`:
1. "Top level" declaration: the *node name* is **required** to be a registered
   dynamic type. Otherwise it's impossible to know what we are trying to parse.
2. "Nested" declaration: Within a struct with an already known type, the *node
   name* is only relevant when the parent node requires named field. If the
  parent node explicitly requires an unnamed field, you **must** use a `-` as
  node name, to elide it.


### Primitive values

Primitive values such as `u16`, `i32` or `String` can be represented as a kdl
values. This means they can appear as *entry values*.

```kdl
NodeName 12 true foo="bar" "more primitive types"
//       ^^ ^^^^     ^^^^^ ^^^^^^^^^^^^^^^^^^^^^^
```

### Newtype structs

structs with a single field, if the field can be represented as a kdl
*entry value*, can itself be represented as an *entry value*.

```rust
struct Newtype(usize);
```

```kdl
NodeName 9999
//       ^^^^
```

This is true even when the field has an explicit name.

```rust
struct NamedNewtype {
  inner: usize,
}
```

```kdl
NodeName 9999
//       ^^^^
```

Finally, this works recursively: the newtype itself can be represented as a
kdl *value*, therefore any newtype wrapping it can be represented as kdl
*values*.

```rust
struct NamedNestedNewtype {
  inner: Newtype,
}
```

```kdl
NodeName 9999
//       ^^^^
```

If you want clarity. You can either use a kdl type declaration or build
newtypes the same way as structs with primitive fields (just check the next
section).

```kdl
NodeName (NamedNestedNewtype)9999
//       ^^^^^^^^^^^^^^^^^^^^
```



### Struct with primitive fields

```rust
struct SimpleFields {
  first_field: u8,
  second_field: String,
}
```

A struct with only primitive fields (ie: can be represented as kdl *value*) can be
declared as a kdl *node*. The *name* of the *node* usually is the rust type name,
fully qualified or shortened. The fields are *parameters* of the *node*.
*Parameters* can be declared in arbitrary order.

```kdl
SimpleFields second_field="Hello World" first_field=34
```

It is possible to ommit the field name and exclusively use *arguments*. Hybrid
argument/parameter declarations are not supported.

```kdl
SimpleFields 34 "Hello World"
```

The arguments must appear in the same order as the rust declaration order.
Otherwise a type mistmach error will be raised.

```kdl
// WARNING: this is an ERROR
SimpleFields "Hello World" 34
// It will cause a "TypeMismatch" error
```

It is also possible to represent the fields as *children nodes*. The *child
node* *name* will be the field name, while its first argument will be the 
field content:

```kdl
SimpleFields {
  second_field "Hello World"
  first_field 34
}
```

Note that this also works with newtype-style structs. Newtypes are not limited
to value position.

```kdl
NamedNewtype inner=9999
```

### Structs with compound fields

```rust
struct CompoundFields {
  first: Vec<String>,
  second: SimpleFields,
  third: u8,
}
```

Structs with fields that cannot fit in a kdl *value* must have their fields
declared as children. 

```kdl
CompoundFields {
  first "hello" "world"
  second first_field=34 second_field="Hello world"
  third 3
}
```

Note that the type name of the field is replaced with the field name. If you
want type-checking regardless, use the kdl type declaration syntax. If the field
type doesn't match the declared type, a type mismatch error will occure.

```kdl
CompoundFields {
  first "hello" "world"
  (SimpleFields)second first_field=34 second_field="Hello world"
  third 3
}
```

It is possible to mix kdl *parameter* field declaration with *node* field 
declaration.

```kdl
CompoundFields third=3 {
  first "hello" "world"
  second first_field=34 second_field="Hello world"
}
```

The advantage of the newtype-to-value shorthand shows when the newtype is
used in another struct.

```rust
struct NewtypeField {
  first_field: Newtype,
  second_field: String,
}
```

Since you can represent `Newtype` as a value, you can use the primitive fields
struct syntax:

```kdl
NewtypeField 9999 "Hello World"
```


### Anonymous tuples

Tuples work like conventional struct, appart that the node name is always `Tuple`
and the field declaration is always sequential.
```rust
type MyTuple = (u8, SimpleFields, String)
```
```kdl
Tuple 25 {
  - first_field=34 second_field="Hello world"
  - "Tuple String"
}
```

Note that the *children* node names are ignored (unless they have a type name).
By convention, we use the dash (`-`) 

```kdl
Tuple 25 {
  CompoundFields first_field=34 second_field="Hello world"
  String "Tuple String"
  // WARNING: type ERROR, because `CompoundFields` does not
  // match the `SimpleFields` expected type.
}
```


### Struct tuples

Struct tuples work like tuples, but the *node name* may be relevant as a top
level declaration.

```rust
struct Fancy(String, u32);
```

```kdl
Fancy "Hello" 9302
```

### Vec & HashMap

#### Vec

Types implementing `DynamicList` are represented as a list

```kdl
Vec 1 2 3 4 5 6 7 8 9 10
```

#### HashMap

Types implementing `DynamicMap` are represented as a key-value pairs. The keys
are limited to `String` types for now.
```kdl
HashMap one=1 two=2 three=3 four=4 five=5
```

#### Compound types

If the list or map contains a compound type in the style `Vec<Fancy>`, it must
be declared as children node.


```kdl
// Vec<Fancy>
Vec {
  - "One thousand" 1000
  - "Two thousand" 2000
  - "Three thousand" 3000
  - "Four thousand" 4000
}
// HashMap<String, SimpleFields>
HashMap {
  ten 10 "Commandments"
  seven 7 "Dwarves"
  five 5 "Fingers"
  four 4 "Cardinal directions"
}
```

Note that it also works with value types:

```kdl
// Vec<u8>
Vec {
  - 1
  - 2
  - 3
  - 4
}
```

### Newtype of compound types

Newtypes do not only work with values, they also work with compound types.

```rust
struct VecNewtype(Vec<String>);
```

```kdl
VecNewtype "one" "two" "three" "four" "five"
```


### Typing

KDL support optional type information. If you want to make sure the *values*
you are declaring are transformed in the right kind of rust data structure, you
should add explicit typing to your *nodes* and *values*.

Note that this will only allow `bevy-reflect-deser` to reject mismatching types.


[template-kdl]: ./../template-kdl/README.md
[miette]: https://crates.io/crates/miette
[examples]: ./examples
