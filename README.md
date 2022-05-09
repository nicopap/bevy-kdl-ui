# Bevy KDL UI

Crates to provide smooth declaration of bevy UI:
* [template-kdl]: A serialization format piggy-backing on KDL, adding a
  powerful yet minimalist macro expension system. The API provides spans
  to attribute nodes to their proper position in the source reader.
* [bevy-reflect-deser]: A deserialization format using bevy_reflect to 
  get a `Box<dyn bevy_reflect::Reflect>` out of any deserializable struct.
  Provides great error reporting based on the [miette] library.
* [bevy-kdl-scenes]: An extension of [bevy-reflect-deser] to support creating
  bundles and collections of entities; A bevy plugin to load and hot-reload
  scenes declared in .kdl files; A system to link references to external
  kdl files.
* [bevy-kdl-ui]: Collection of macros for use with [bevy-kdl-scenes] and
  [bevy_ui] to write native bevy uis without the hassle.

## Getting started

See the relevant sections in the respective crate READMEs.

[template-kdl]: ./template-kdl/README.md
[bevy-reflect-kdl]: ./bevy-reflect-kdl/README.md
[bevy-kdl-scenes]: ./bevy-kdl-scenes/README.md
[bevy-kdl-ui]: ./bevy-kdl-ui/README.md
[bevy_ui]: https://docs.rs/bevy_ui/latest/bevy_ui/
[miette]: https://crates.io/crates/miette

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

## Function design

A "function" node that binds a node name to an arbitrary transformation
into another node. Whenever a node with the bound name is found, it will
be transformed into the child node of the function declaration, according
to its arguments and parameters.

Terms:
* `declaration site`: Where the function is defined
* `call site`: Where the function is used
* `body`: The node into which the function expands at call site, defined at
  declaration site.
* `fargument` (actual parameter) is the value provided to the function as argument
  at call site
* `fparameter` (formal parameter) is the name of variables to which a caller
  can assign arguments in a function.
  
I prefix those with `f` as to distinguish them from the kdl naming convention.

All nodes but the last in a top level KDL document are function nodes. The binding
names is the node name, **fparameters** are the entries and children nodes of the function
node. The **body** of the function is the last child node of the function node.

All **fparameters** have a "name". All occurences of a fparameter name in the body of the
function will be replaced by the farguments of the function.

To use a function node, you **call** it by simply using a node with the same name. Entries
and children of a called function are then passed to the function as **farguments**.
The call site will then be replaced by the body of the function, where all occurences
of fparameter names will be replaced with the farguments.

The document (or children) of the `fn` are additional `fparameter`/`fargument` pairs
so that it's possible to declare more complex default `fargument`s.

The last child of the children is the `body` of the function. That node will replace
all occurences of fparam names in the rest of the kdl document by their respective
farguments. This implies restrictions on what an input parameter can be:
* a `fparameter` that appears in node-name position must either have a string or
  node `fargument`.
  * If string, the name gets replace.
  * If node, the whole node gets replaced, including children and entries.
    (it might become an error for nodes in the body with `fparameter` name
    to have entries or children in a future version)
* a `fparameter` in key position of a parameter entry must be a string
* a `fparameter` in entry position must be a value
* a `fparameter` in value position of a parameter entry must be a value

```kdl
washing-machine "load" "brand" type="top-loading" {
  motor "Motor" .wattage=225 .type="induction" .noise=12.0
  WashingMachine .load="load" .brand="brand" {
    motor
    MachineType .type="type" .is_electric=true
  }
}
// later, nodes in the form `washing-machine "bleh"` will be expanded
Laundry {
  .machine_list {
    washing-machine load=3.0 brand="Miele"
    washing-machine load=1.0 brand="Bosch" type="front-loading"
    washing-machine 1.0 "Arctic" type="front-loading"
    washing-machine 8.2 "GE" {
      motor "Motor" .wattage=3434 .type="steam" .noise=20.0
    }
  }
}
// becomes
Laundry {
  .machine_list {
    WashingMachine .load=3.0 .brand="Miele" .type="top-loading" {
      Motor .wattage=225 .type="induction" .noise=12.0
    }
    WashingMachine .load=1.0 .brand="Bosch" .type="front-loading" {
      Motor .wattage=225 .type="induction" .noise=12.0
    }
    WashingMachine .load=1.0 brand="Arctic" .type="front-loading" {
      Motor .wattage=225 .type="induction" .noise=12.0
    }
    WashingMachine .load=8.2 brand="GE" .type="top-loading" {
      Motor .wattage=3434 .type="steam" .noise=20.0
    }
  }
}
```

### Function implementation

At first, I implemented nodes resulting from function application as a
different kind of nodes.

But it doesn't make sense! A "normal node" (ie: not in a function invocation)
is the same as a node in a function invocation, but where the argument list
is completely empty. So we basically have to replace the implementation of 
`Spanned*` to account for the new substitution environment.

I struggled several day until I reached that conclusion. Which in turn made
it real easy to finally implement functions.

#### Variable scopping

Now we introduced the concept of bindings and expensions, a few rules are
necessary:

* When recursively expanding functions, variables should be properly managed
* When definining a `fn`, we should "capture" an immutable binding environment 

**binding set**: the set of variables that are declared and can be used in the
current scope. The difficulty comes from the fact that the binding set at the
call site of a function is different from the binding set at the declaration site.
And when expanding the function with its parameter, we must use the declaration
site binding set _for the body_, while using the call site binding set for the
nodes passed as argument at the _call site_.

This means the scope and which binding set is active is tied to which node we are
looking at right now.

At first I thought to add it to the `Context` struct in `visit.rs`, but a
problem I was getting is that I can't "push" and "pop" the binding set into the
context in a fool-proof way. I'll have to make sure everywhere I enter and leave
a scope to manually add and remove the binding set from the stack.

The `NodeThunk` pairs a binding set with a node, this way, when walking
the node, the thunk will properly retrieve from the set the proper binding to
expand correctly the node elements.


## TODO

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
* Consider non-consuming parsing (after all, the only values we really copy are the
  primitive types & strings, and we are already cloning them a bunch) (done??)
* proper span-based error reporting (done)
* Spanned smart pointer that forces updating offset when accessing a Kdl type (done)
* Refactor
  * `fns` => `prefab`
  * Make sure the documentation reflects the actual syntax
  * `Call*` => `*Thunk`
  * Formalize and document difference between `Context` and `Bindings`
  * Rename `Bindings`
  * Remove dependency on pointers, own shit so that it might be possible
    to use multiple files
* Resolve the "variable scopping" problem.
* Document `fns` (mostly copy the section on top here)
* FIX error message for too many fields in tupleStruct
* FIX that we accept .N="foo" where N is out of bound for tupleStruct
* FIX that we accept field reference mixup in tupleStruct
* FIX TupleStruct .N= not doing anything
* Load more than one node
* detect and parse `fn` nodes (done)
* Expand `fn` nodes in the last node of file (done)
* Expand `fn` nodes in other nodes (done)
* Actually use invocation arguments in `fn` expension
* `expand` meta-node
* add a `bundle` node so that it's possible to define multiple
  components at a time
* ?? Consider using "field:" instead of ".field" for performance (easier to remove from
  the end than the start)
* Add "kdl markers" to nodes spawned, so that it might be eventually
  possible to round-trip the world, for 
* Read and add to assets the last node in the kdl file
* Check for `Added<Handle<Cuddly>>` and add scenes
* ?? Enable usage of kdl type specifiers and checking against expected values
* ?? type-directed field assignment (arbitrary ordering of fields in kdl file
  as long as it is possible to guess to which field a node element belongs based
  on either field name or type)
* ?? Non-String Map access.
* ?? Handle Enum

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