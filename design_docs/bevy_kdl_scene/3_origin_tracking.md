## Tracking source of components

We want to associate with each entity (probably each _component_, even _each
field_) the source file and span responsible for it. This is challenging in
many ways.

Tentative name for a component for tracking source: `Marker`.

Why is it challenging:

* Storing in a single component information that has to do with many different
  components. I think `Marker` should use reflection and store a collection of
  source path
* With template system, information goes through many different places before
  resulting in a final value. Which one should I take?
* Loss of information in the various `template_kdl::read_doc` and
  `bevy_kdl_deser_reflect` methods.
* Keeping up-to-date