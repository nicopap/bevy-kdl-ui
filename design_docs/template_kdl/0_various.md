## Improving API

I want to limit the API surface so that only the minimum is exposed. Possible
options:
* Enable definiting `Declaration` without recourse to parsing some KDL
* Remove `DocumentThunk` from public API by having `children` return
  nodes.

## Recursive behavior

So we artifically prevent recursive calls by storing a `binding_index`. But why?
Well, currently it's impossible to have terminal conditions, so it's better to
make it hard to make recursion, since all forms of recursion would be infinite.

## Better Span API

Issue: currently, `Span` is very annoying and difficult to deal with, goes into
pattern matching, requires mapping over, etc. How could we smooth it out?

* special `map`, `flat_map` etc. that transparently handle `Span` when it's inside
  `Result`, `MultiResult` and `Option`.
* Instead of having `Spanned` be a wrapper struct, have it be a `trait`, so that it
  doesn't poison the types

## Foreign definitions API (or "Scopes")

We want:
* Ability to refer to user-provided or `export`-returned templates
* For those templates to be able to refer to their local scope without
  interference from templates defined somewhere else.
  
Now that we have mappable-rc, we can simply create a `Marc` over the subslice
of bindings we need in `Context`, rather than storing an indice.

We'd also love to make the foreign definitions blend with the local ones, so
that it's easier to manage.

We'd like for a sort of "dependency graph" so that it's possible to dynamically
reload the proper templates and scenes.
