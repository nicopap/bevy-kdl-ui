## template-kdl

### User declared list of additional declarations

We want to be able to parse and return a set of `Declaration` so that you can
use it afterward with different "Deserializers".

Problem: Current code relies heavily on shared pointers on KdlDocument for the
declaration system.

Why? Each node/doc/entry is associated with a span with Spanned*, we store the
span with the pointer to the n/d/e so that whenever we call the template, it's
possible to recall the span with it. We also bundled the Context with the 
declaration with *Thunk.

Solution: We should use a different indirection method for both the context and
the n/d/e span in `Declaration`.

How? Forget it for now.

### Create a `Deserializer` that encapsulates completely parsing

We want to minimize the API surface, so that the end user ideally doesn't have
to deal with `Kdl*` stuff. Also, everyone knows how to use `serde`. And it is
a good abstraction method to enable usage of this library beyond just the needs
of `bevy-kdl-scene`.

**Problem 1: How do I create and store Spanned\*?**

Solution: Problem for later. For now, just accept and store a &KdlDocument, we
then can use the API we already wrote without changing it.

**Problem 2: How do I handle int conversions?**

Currently int conversion is managed by `bevy-reflect-deser` with detailed error
messages tied to type info. What would we lose by moving it to `template-kdl`?

Looking at serde example, they use a parse_int<T: From<i64>>() and feeds it to
the visitor.

**Problem 3: Which data model to use?**

The [serde data model] is very different from the kdl one. I however still want
to keep more or less the same walking system. How do other crates do it?
* [serde_kdl][serde_kdl]: copes out completely, basically doing what bevy_proto
  does, see [serde_kdl doc].
* [knuffel]: Gives up entirely, the macros specifically asks the user which
  fields of a struct should represent what in the kdl file.

**FORGET IT** I don't want to deal with struct shape in this crate. It is not
the goal of this crate to provide a way to translate KDL documents into rust
structs. I only want to provide a transformation layer on top of the kdl format.

### Improving API

I want to limit the API surface so that only the minimum is exposed. Possible
options:
* Enable definiting `Declaration` without recourse to parsing some KDL
* Remove `DocumentThunk` from public API by having `children` return
  nodes.


## bevy-reflect-deser

### Newtype typing

Problem: We want newtypes to be transparent. We should be capable of expecting
several different types.

Expected types are passed to `ValueExt::into_dyn` which itself forwards the
expected values to `KdlConcrete::into_dyn` and `NodeThunkExt::into_dyn`.

* `KdlConcrete::into_dyn` is used in `ValueBuilder::new_dynamic` and
  `ValueExt::into_dyn`
* `NodeThunkExt::into_dyn` in `convert_doc` and `ValueExt::into_dyn`
* `ValueExt::into_dyn` in `Wrapper::add_field`, both `Builder` implementations

Currently, KdlConcrete is where we list all expectable types for something. Which
is not the right place. We should instead accept a _list of expected types_ and
work based on that.

Since `into_dyn` is mostly called in `add_field`, it means we have to create the
list of expected types there.

**How to generate the list of expected types?**: `newtype_wrapping` function in
`dyn_wrapper` module. ⇒ **problem**: how to reconstruct the actual type from
there?

**Solution**: We crate a `ExpectedType` struct that both goes down the hierarchy
and goes back up.

**Problem**: Current implementation ignores completely intemediate values, so that
it's impossible to declare the newtypes as a complete struct.

**Solutions**:
* Go down hierarchy of wrapped type, and go back up when first matching
  => Problem with that is now we break declarations like `Newtype "foo" "bar"`
     because it's incompatible with `Vec<String>` when `struct Newtype(Vec<String>)`
  => **solution?**: maintain a list of synonyms? => Makes too much change to existing
     code
  => **not solution**: only care about transformation where the last value is primitive
     (we also want this to work with newtyped complex data)
  => **solution?**: Have a special error type that happens only when the type specifier
     and expected are not matching
  => **solution?**: Some sort of recursion
     => Current algo is recursive
     => Do it this way: Go bottom up, by trying lowest, then failing, then go one up, try
        etc until it works or not!
     => The reason it's broken right now is because we accept values where we shouldn't.
  
What needs to be considered identical?
```kdl
NamedNestedNewtype inner=9999
NamedNestedNewtype 9999
NamedNestedNewtype { Newtype 9999 ; }
(NamedNestedNewtype)Newtype 9999
(NamedNestedNewtype)Newtype inner=9999
(NamedNestedNewtype)9999
Newtype 9999
Newtype inner=9999
9999
```

But that is only when parsing a `NamedNestedNewtype`, otherwise it's not good.
=> **Solution?**: in `type_info`, accept a list of expected types?
   => Nuuhhh, that's what we decided against in a past life (we are *generating* the
      list in that function)

Currently: only context is the ExpectedType. It is indeed the one I need, but it seems
to be lost at a critical point where it is required to know I need to match differently
the type I'm trying to deserialize

**Implementation**: Most of the problems I had encoutered was related to the fact that
primitive types in node position would accept field names or multiple arguments. Which
when the most inner type was a value type, would mark as "valid" nodes with fields or
multiple entries/children. This prevented detecting nodes as invalid for some primitive
types.

On top of that, the previous algorithm didn't at all support declaring the newtype as a
struct, it only accepted the most unwrapped version. Now, the `ExpectedType::into_dyn`
method tries a higher level of nestedness if the most inner type didn't work.


### Error message spam

```
Error: 
  × There is no such registered type: one
   ╭────
 1 │ NewtypeContainer nine=9 eight=8 seven=7 six=6 five=5 four=4 three=3 two=2 one=1
   ·                                                                           ────
   ╰────
  help: Try adding it to the type registry with `reg.register::<one>()`.
Error: 
  × Kdl declaration has type `int(1)` but rust type `HashMap<String, VeryNewtype>`
  │ was expected
   ╭────
 1 │ NewtypeContainer nine=9 eight=8 seven=7 six=6 five=5 four=4 three=3 two=2 one=1
   ·                                                                               ─
   ╰────
  help: You probably meant to declare a HashMap<String, VeryNewtype>.
```

There are multiple problems with those error messages:
* Because it expected `NewtypeContainer` which is a TupleStruct, it reads the name
  of entries as the type declaration. But they are not types, so it fails to "find
  it in the register". And displays a bad "helpfull" error message.
* It repeats 2×9 times the same message, which is too much!
* It suggested to add `one` as a type to register. But `one` is not a type! How
  could we avoid giving that suggestion for things that are not types?


#### Solution

Difficult because parsing is general and this requires a specific solution.

* fold all identical error messages from the same level into one.
* in `type_info` raise a "not a type" error rather than "not registered"
  if we can't find the `declared` type AND not uppercase AND ≠ "u8, u16 etc."

### Anonymous tuple aka type-unaware visit

We want to be able to declare tuples such as:

```kdl
"(NamedNestedNewtype, NamedNewtype, Newtype)" {
  - (NamedNestedNewtype)1234
  - (NamedNewtype)2345
  - (Newtype)3456
}
```

can be expressed without explicitng all the types in the node name, so that:

```kdl
Tuple {
  - (NamedNestedNewtype)1234
  - (NamedNewtype)2345
  - (Newtype)3456
}
// or 
Tuple {
  NamedNestedNewtype 1234
  NamedNewtype 2345
  Newtype 3456
}
```

is possible.

However, the current design assume complete awareness of the type of nodes,
because only the top level node has a prefectly unknown type. This requires
being able to navigate the nodes "blind" which currently is partially possible
but accumulates errors in the `MultiResult`.

**How to design this?**: `ExpectedType` should optionally be able to not expect
anything. And `dyn_wrapper::type_info` should be able to correctly the case where
we tell it to not expect anything: it's not an error to not expect anything if we
could deduce the type from the `declared` argument.



[serde data model]: https://serde.rs/data-model.html
[serde_kdl]: https://crates.io/crates/serde_kdl
[serde_kdl doc]: https://github.com/Br1ght0ne/serde_kdl/blob/5cf480b27aa0c8d7fa688d0faebcc0d56f269530/src/node.rs#L12-L39
[knuffel]: https://crates.io/crates/knuffel
