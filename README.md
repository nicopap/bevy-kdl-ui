# Bevy KDL UI

Crates to provide smooth declaration of bevy UI:
* [template-kdl]: A serialization format piggy-backing on KDL, adding a
  powerful yet minimalist macro expension system. The API provides spans
  to attribute nodes to their proper position in the source reader.
* [bevy-reflect-deser]: A deserialization format using bevy_reflect to 
  get a `Box<dyn bevy_reflect::Reflect>` out of any deserializable struct.
  Provides great error reporting based on the [miette] library.
* [bevy-kdl-scene]: An extension of [bevy-reflect-deser] to support creating
  bundles and collections of entities; A bevy plugin to load and hot-reload
  scenes declared in .kdl files; A system to link references to external
  kdl files.
* [bevy-kdl-ui]: Collection of macros for use with [bevy-kdl-scene] and
  [bevy_ui] to write native bevy uis without the hassle.

## Getting started

See the relevant sections in the respective crate READMEs.

[template-kdl]: ./template-kdl
[bevy-reflect-deser]: ./bevy-reflect-deser
[bevy-kdl-scene]: ./bevy-kdl-scene
[bevy-kdl-ui]: ./bevy-kdl-ui
[bevy_ui]: https://docs.rs/bevy_ui/latest/bevy_ui/
[miette]: https://crates.io/crates/miette

## TODO

- [X] Load a single node from a kdl
- [X] Deserialize into some dynamic struct
- [X] Owned DynamicKdlStruct
- [X] Deserialize Reflect components 
- [X] Figure out how to deserialize integral numbers coherently
- [X] Refactor owned_visit as to couple more tightly with TypeInfo and
  separate TupleStruct constructor from truct constructor
  Also makes error reporting and type checking easier.
- [X] handle top level value types
- [X] Allow newtypes to be declared in "KdlValue" if it is wrapping something
      that can
- [X] Add Vec & HashMap handling
- [X] Extend kdl-type match checking to everything
- [X] Consider non-consuming parsing (after all, the only values we really copy are the
      primitive types & strings, and we are already cloning them a bunch)
- [X] proper span-based error reporting
- [X] Spanned smart pointer that forces updating offset when accessing a Kdl type
- [X] Implment newtype unwrapping for compound types
- [X] Refactor
  - [X] `fns` => `template`
  - [X] Make sure the documentation reflects the actual syntax
  - [X] `Call*` => `*Thunk`
  - [X] Formalize and document difference between `Context` and `Bindings`
  - [X] Rename `Bindings`
- [X] Resolve the "variable scopping" problem.
- [X] Document `fns` (mostly copy the section on top here)
- [X] `deser`: Implement all documented features that are currently commented-out in
      integration tests.
  - [X] Anonymous tuples
  - [X] Struct with named field but not named in kdl file
  - [X] Hashmap tuple form
- [X] Migrate `NodeThunkExt` to `template-kdl`
- [X] FIX error message for too many fields in tupleStruct
- [X] FIX that we accept .N="foo" where N is out of bound for tupleStruct
      (actually removed the feature)
- [X] FIX that we accept field reference mixup in tupleStruct
- [X] FIX TupleStruct .N= not doing anything (actually removed the feature)
- [X] Load more than one node
- [X] detect and parse `fn` nodes
- [X] Expand `fn` nodes in the last node of file
- [X] Expand `fn` nodes in other nodes
- [X] Actually use invocation arguments in `fn` expension
- [X] Make all `template-kdl` examples pass
- [X] `expand` meta-node
- [X] Formalize and list the bevy-reflect-deser format.
- [X] Add the README example in bevy-reflect-deser to rustdoc.
- [X] ?? Consider using "field:" instead of ".field" for performance (easier to remove from
      the end than the start) (now it uses plain name)
- [X] Fix broken links in READMEs
- [X] Spaces and non-string map keys
- [X] ?? Enable usage of kdl type specifiers and checking against expected values
- [X] ?? Non-String Map access.
- [ ] Add a `if_equal foo bar` template built-in.
- [ ] bevy-kdl-scene features:
  - [X] in teplate-kdl: remove dependency on pointers, own shit so that it might be
    possible to use templates in a concurrent context. (see [arc-decision])
    - [X] Requires migrating to `Arc` everything instead of storing references. 
  - [X] Update template-kdl to return an alternative between `Vec<Definition>` and
    `NodeThunk` when parsing a file
  - [X] Add name rebinding to `export`
  - [X] Update template-kdl to accept a list of template definitions to parse a file with.
  - [ ] add a `components` node so that it's possible to define multiple
        components at a time
  - [ ] Add "kdl markers" to nodes spawned, so that it might be eventually
        possible to round-trip the world
  - [ ] Read and add to assets the last node in the kdl file
  - [ ] Check for `Added<Handle<Cuddly>>` and add scene
- [ ] Proper error reporting and handling
  - [ ] Error handling in `template-kdl`
  - [ ] Deduplicate similar errors in `bevy-reflect-deser` (in
        dyn_wrappers::Builder::new_dynamic)
  - [ ] Add context to field errors (encompassing struct, alternative possible names etc.)
- [ ] Refactoring
  - [ ] !! Implement a "serde-like" API on top of NodeThunk, because I bet I can clean up
        thousandfold the `bevy-reflect-deser` crate if I expose the right API.
  - [X] Remove `EntrySizes` and `NodeSizes`, since the string len op is pratically identical
        to looking up a u32.
  - [ ] Use a context with interner & slotmap for definitions in template-kdl rather than
        stash everything in a thunk (for performance)
  - [ ] Flatten the many different binding tables
  - [X] Have a `SpannedValue` that replaces the clone-heavy 'Spanned<KdlValue>'
  - [ ] Use a string interner for `template-kdl`
  - [X] Rename `FooThunk` to `Foo` in `template-kdl`
  - [ ] rename `dyn_wrappers` in `bevy-reflect-deser`
  - [ ] Move `MultiResult` into its own crate.
  - [ ] Redesign Span and MutliResult to have a smoother API (maybe use a trait)
  - [ ] `deser`: do not store type names as string, rather just the TypeId. And
        get back the string when building the final `ConvResults`
- [ ] ?? Things to consider but might not be implemented.
  - [X] ?? Recursive `template-kdl` by removing `binding_index` field from `Binding` struct
  - [ ] ?? type-directed field assignment (arbitrary ordering of fields in kdl file
        as long as it is possible to guess to which field a node element belongs based
        on either field name or type)
  - [ ] ?? Consider having a generic template parser rather than one that depends on Kdl
        it would just wrap another Deserializer. => Check [design decision][deser-decision]
  - [ ] ?? Make bevy-reflect-deser independent from tempalte-kdl
  - [ ] ?? Use the `NodeThunkExt` API to create a `kdl-serde`
- [ ] ?? Handle Enum (waiting on https://github.com/bevyengine/bevy/pull/4761)

[arc-decision]: ./dev-resources/decisions#user-declared-list-of-additional-declarations
[deser-decision]: ./dev-resources/decisions#create-a-deserializer-that-encapsulates-completely-parsing

## Why

This was inspired by someone posting in the bevy discord a link to
kdl and shortly afterward someone asking about UI hot reloading.

I immediately felt in love with KDL. It's very close (but far superior)
to the monstruous bevy ui macros I've used precedently.

### Other works

[bevy-proto][bevy-proto] is basically the same thing, but a lot more
verbose. I think the node-based nature of kdl helps reduce boilerplate
a massive deal, and is specifically more 

[bevy-proto]: https://github.com/mrgvsv/bevy_proto

## License

All crates in this repository are

Copyright © 2022 Nicola Papale

This software is licensed under either MIT or Apache 2.0 at your leisure. See
LICENSE file for details.