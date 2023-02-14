## Newtype typing

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


