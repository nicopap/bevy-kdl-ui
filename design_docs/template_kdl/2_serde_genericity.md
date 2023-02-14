## Create a `Deserializer` that encapsulates completely parsing

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
* [serde_kdl]: copes out completely, basically doing what bevy_proto
  does, see [serde_kdl doc].
* [knuffel]: Gives up entirely, the macros specifically asks the user which
  fields of a struct should represent what in the kdl file.

**FORGET IT** I don't want to deal with struct shape in this crate. It is not
the goal of this crate to provide a way to translate KDL documents into rust
structs. I only want to provide a transformation layer on top of the kdl format.


## Make a Deserializer Wrapper

Idea: Do not depend at all on kdl for `template-kdl`, but only depend on serde.
This way it works with any sort of serialization format.

This needs thinking more thouroufully of the API for templating. The current
idea is:
* Provide an API to build `Bindings`. It should be able to own `Deserializer`
  for substitution.
* Completely decouple declaration from substitution. Ability to construct a
  wrapping `Deserializer` from an initial deserializer and bindings.
* Rethink API so that it makes more sense to use it in a generic way.

### Rethinking the template call

**Problem**: current template call is highly dependent on KDL, we want
something that makes sense in serde data repr.

Currently: We have a mix of by-name and by-index arguments. **we
automatically use by-index for children nodes**, while for entries, we
only do it if the name is omitted. This is very convinient, because it
let me declare node arguments as direct child of the template call node.
This avoids having to wrap every node arguments into another node.

Ideally, we could extend the `Deserializer` API to have a "is this value
replaceable?" method, but I don't think it's possible, since it doesn't
have at any point access to the value. It also precludes re-using already
existing deserializers.

It would be fine to ignore the issue with KDL, since it's pretty exceptional,
and write my own Deserializer for it.


[serde data model]: https://serde.rs/data-model.html
[serde_kdl]: https://crates.io/crates/serde_kdl
[serde_kdl doc]: https://github.com/Br1ght0ne/serde_kdl/blob/5cf480b27aa0c8d7fa688d0faebcc0d56f269530/src/node.rs#L12-L39
[knuffel]: https://crates.io/crates/knuffel
