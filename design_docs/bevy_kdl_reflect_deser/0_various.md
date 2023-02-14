## Anonymous tuple aka type-unaware visit

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


## ser/deser entities

**Problem**: We want human-readable (and editable) entity names in the scene
file. The default reflect serialization is wonk.


