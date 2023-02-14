# Bevy KDL UI

Helpers to manage your UI using [bevy-kdl-scene].

This library does:
* Load files formatted in the [kdl document language][kdl].
* Transparently load custom `Reflect` components defined by user or
  3rd party library.
* Let user specify "prototype" nodes to use in further definitions,
  providing itself a minimal set of nodes useful to define UIs.
* Let user specify custom "adaptators" functions to translate kdl
  fields into setters for specific components.
* Can reference "prototypes" accross multiple `.bui.kdl` files.
* Support transparently hot-reloading.
* Provide its own set of components to points to the origin
  of ECS entities to the kdl definition for purposes only the users
  could guess.
* Ostentatiously schmaltzy API.

[kdl]: https://kdl.dev

[bevy-kdl-scene]: ../bevy-kdl-scene
