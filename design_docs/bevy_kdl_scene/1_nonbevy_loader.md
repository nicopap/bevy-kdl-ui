## Kdl template manager

The bevy asset system is too limited to manage template dependencies, so we
implement our own. Requirements are:

- [ ] Can discriminate between "export" and "final" files
- [ ] Knowns a graph (DAG/latice) of file dependency
  - [ ] Can read imports to know dependencies of a file.
- [ ] Async load of templates.
  - [ ] Use a task pool to load stuff
  - [ ] Has awareness of load state of graph nodes.

### Async

This is basically a complete rewrite.

One problem with the current loader is that it blocking, so it will
probably cause stuttering. But I can't use the `bevy_asset` loader because it
doesn't handle dependencies properly.

Seemed easy at first to do the loading:

Similarly to the `examples/async_tasks/async_compute.rs` example, I thought I
could just use tasks. I would store the task in a component and pool it.
Issue is I need to spawn more entities with more tasks for each task
(dependency on other templates) but I can't hold concurrently a lock on the
world.

An alternative would have been to use something similar to the `necking`
(and therefore bevy-native) hot-relaoding system. Where I spawn a thread to
which I send requests and it responds with messages on a shared channel. The
messages would be pooled each frame.

Other alternative: the spawned process may respond with the loaded value or a
"re-run me with given dependencies loaded."