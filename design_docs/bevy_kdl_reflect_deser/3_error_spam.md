## Error message spam

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


### Solution

Difficult because parsing is general and this requires a specific solution.

* fold all identical error messages from the same level into one.
* in `type_info` raise a "not a type" error rather than "not registered"
  if we can't find the `declared` type AND not uppercase AND ≠ "u8, u16 etc."

