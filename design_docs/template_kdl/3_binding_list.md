## Binding list issue again

I tried to use a `Arc<[Binding]>` or 'Marc<[Binding]>' for the binding list, but
it wouldn't do it. Issue is that it required each `Binding` to have a reference
to the list itself in which it is. Even if all I really want is just a reference
to the subslice that of bindings just before this one.

I opted to make a linked list of `Binding` Where `Binding` is a node, and has a
`bindings: Option<Arc<Binding>>` (aliased to 'OtherBindings'). This simplifies
implementation, because the bindings are fully associated with the thunks, and
I don't have to keep around (and in sync) a context to interpret the templates.

However, when performance is an issue, I should switch to using, for one, a
string interner instead of a bunch of `Marc<str>`, and for two using a context
where bindings are not strings but rather indices into the context, which would
supposedly store the bindings in contiguous memory.

