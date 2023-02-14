## User declared list of additional declarations

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

Update: now is the time to think about it.

So I have an initial owned `KdlDocument`. But all the '*Thunk' stuff hold
references, because it only visits the 'KdlDocument' when consumed.

Alternatives:
1. Clone everything and keep ownership all the time
2. Add `Arc`s everywhere.

(1) is not memory-performant and seems generally wastefull. (2) Is actually
impossible, because you still need ownership of the value to create the `Arc`.
A workaround would be to do a "pre-processing" pass that converts the
`KdlDocument` and its node to a proxy struct that holds `Arc` to the values
rather than the value itself. But this is complex and adds a lot of code.

After some research, I found `mappable-rc` which seems to enable creating
projections of `Arc`s to fields of a struct which is exaclty what I need.

## Declaring the list of dependencies

**Problem**: we want in `bevy-kdl-scene` to premeptively load template declaration
files used in scenes. If we don't, the templates used inside the files won't
be expanded.

### The `import` node

I think it's necessary to explicitly declare the bindings required for import.

We define an `import` node. How it looks in the API:

- `template_kdl::read_document` now requires a `RequiredBindings`
- To get the `RequiredBindings`, we need to call `template_kdl::get_imports`
  `template_kdl::get_imports` returns a `Imports`
- `Imports::bindings` returns the `RequiredBindings` necessary for
  `read_document`
- As argument to `Imports::bindings`, we have a `ExportedTemplatesList`
- `ExportedTemplatesList` aggregates templates from many different documents
- The return value of `read_document` is an enum of either a computed node or
  a list of `ExportedTemplates`.
- This is a recursive process technically.

