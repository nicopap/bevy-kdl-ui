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



[serde data model]: https://serde.rs/data-model.html
[serde_kdl]: https://crates.io/crates/serde_kdl
[serde_kdl doc]: https://github.com/Br1ght0ne/serde_kdl/blob/5cf480b27aa0c8d7fa688d0faebcc0d56f269530/src/node.rs#L12-L39
[knuffel]: https://crates.io/crates/knuffel
