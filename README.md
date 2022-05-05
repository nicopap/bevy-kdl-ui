# Bevy KDL UI

## Type-directed redesign

TODO: this section should be added to the code documentation.

Need to add the expected type in the "Context"

### What can we expect?

* Fields: (implicit or explicit)
* That's it.

How to use expectation:
* should help "link" nodes to struct fields.
* Could even guess exactly which struct field based on type
  * Note: only when all field types are distinct, or cannot be cast
* Cast (float) or (int) into the actual expected concrete type (u64, f32)

Is it possible to not be able to expect anything?
* Only at root location I guess.

TODO:
* Load a single node from a kdl (done)
* Deserialize into some dynamic struct (done)
* Owned DynamicKdlStruct (done)
* Deserialize Reflect components (done) 
* Figure out how to deserialize integral numbers coherently (done)
* Refactor owned_visit as to couple more tightly with TypeInfo and
  separate TupleStruct constructor from truct constructor
  Also makes error reporting and type checking easier. (done)
* handle top level value types (done)
* Allow newtypes to be declared in "KdlValue" if it is wrapping something
  that can (done)
* Add Vec & HashMap handling (done)
* Extend kdl-type match checking to everything (done)
* proper span-based error reporting
* ?? Enable usage of kdl type specifiers and checking against expected values
* ?? Spanned smart pointer that forces updating offset when accessing a Kdl type
* ?? Non-String Map access.
* ?? Handle Enum
* Load more than one node
* detect and parse `fn` nodes
* Expand `fn` nodes in the last node of file
* Expand `fn` nodes in other nodes
* add a `bundle` node so that it's possible to define multiple
  components at a time
* Add "kdl markers" to nodes spawned, so that it might be eventually
  possible to round-trip the world, for 
* Read and add to assets the last node in the kdl file
* Check for `Added<Handle<Cuddly>>` and add scenes
* ?? type-directed field assignment (arbitrary ordering of fields in kdl file
  as long as it is possible to guess to which field a node element belongs based
  on either field name or type)
* ?? feature-gate `bevy_text`

## Special nodes

### Terminology (name of stuff)

I'll use the KDL terms:
* A `node` is an arbitrary hierarchy that must have a name, can have arguments
  and can have children nodes.
* A node `name` is the first element of a node.
* An `entry` is what follows the `name`, it is either an `argument` or a `parameter`
* An `argument` is a bare entry, ie: is not an identifier/value pair `foo=bar`, just
  a value.
* A `parameter` is an identifier/value pair of the form `foo=bar` in entry position.
* A `document` is a series of nodes. The list of children of a node is a document, a
  KDL file is a document. 

The following nodes are referred by their names.

### `fn` nodes

A "function" node that binds a node name to an arbitrary transformation
into another node. Whenever a node with the bound name is found, it will
be transformed into the child node of the `fn` declaration. It is an
error for `fn` nodes to have not exactly one child node.

`fn` node entries are:
* argument at position 1, `binding`: the name by which the function will be
  refered later.
* Any other entries: inputs to the function call to substitute into the child
  node definition. Parameters enables default values.
```kdl
fn binding arg1 arg2 param1=default param2=default {
  other_node {
    arg1 "foo";
    special_node arg2 param1;
    // etc.
  }
}
```

### `bundle` nodes

Put together multiple component nodes to specify a bundle.
```kdl
bundle {
  FooComponent bar=10 baz="hello";
  BarComponent 34 1134;
}
```

### `.[a-z0-9]+` nodes and parameters

All nodes with a name starting with `.` are "field" nodes: ie they represent
a field of their parent. This is also true of parameters.

`.[0-9]*` represent fields of a tuple structs. `.[a-z]\*`, fields of struct
structs. `[a-z]` here is any valid identifier characters in rust, not just
the lowercase letters.


bevy-kdlui is an asset format to load bevy UIs from `.bui.kdl` files.

This library does:
* Load files formatted in the [kdl document language][kdl].
* Transparently load custom `Reflect` components defined by user or
  3rd party library.
* Let user specify "prototype" nodes to use in further definitions,
  providing itself a minimal set of nodes useful to define UIs.
* Let user specify custom "adaptators" functions to translate kdl
  fields into setters for specific components.
* Can reference "prototypes" accross multiple `.bui.kdl` files.
* Support transparently hot-reloading.
* Provide its own set of components to points to the origin
  of ECS entities to the kdl definition for purposes only the users
  could guess.
* Ostentatiously schmaltzy API.

[kdl]: https://kdl.dev

## Usage

A `.bui.kdl` file is a `kdl` file where the last entry is a sort of
scene that can be added to the world.

```kdl
TODO
```

On the rust side, you'll get a `Handle<KdluiNode>` that you can later use
to spawn into the world your UI tree.

```rust
#[derive(Component, Reflect)]
#[reflect(Component)]
enum MainMenuButton {
  Start,
  Options,
  Exit,
}
fn spawn_ui(
  mut commands: Commands,
  asset_server: Res<AssetServer>,
) {
  let main_menu: Handle<KdluiNode> = asset_server.load("main_menu.bui.kdl");
  commands.spawn().insert(main_menu.clone());
  todo!("TODO: revise this")
}
```

You'll get the following:

TODO: screencap of `kdl` file defined on top.

This crate doesn't aim to structure how you declare logic related to your UI.
The current approach is the same as with the native bevy ui. You may be interested
in my [bevy-ui-navigation][bevy-ui-navigation] crate used in combination with marker
components, the code would end up looking like the following:

```rust
fn handle_ui(
  mut events: EventReader<NavEvent>,
  buttons: Query<&MainMenuButton>,
) {
  for event in events.iter() {
    if let NavEvent::NoChanges { from, request: NavRequest::Action } = event {
      match buttons.get(from.first()) {
        // Do things when specific button is activated
        Ok(MainMenuButton::Start) => {}
        Ok(MainMenuButton::Options) => {}
        Ok(MainMenuButton::Exit) => {}
        Err(_) => {}
      }
    }
  }
}
```

[bevy-ui-navigation]: https://github.com/nicopap/ui-navigation

### List of pre-defined prototype nodes

* `fullscreen`: A node with size set to 100%
* `image`: The first argument is the file name of the image
* `text`: The first argument is the text, color and size can be set
  with properties.

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