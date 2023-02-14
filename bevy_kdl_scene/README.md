# Bevy kdl scene

Combines [bevy-reflect-deser] and [template-kdl] to define entire bevy scenes,
supporting templating out of the box!

Features:
* A bevy plugin to load and hot-reload scenes declared in .kdl files
* A system to link references to external kdl files.

## Getting Started

Add this to your `Cargo.toml`:
```toml
bevy-kdl-scene = "0.4.0"
```

## Usage

A scene is a hierarchy of entities. Unlike json, kdl is particularly fit to
represent a hierarchy of entities, as it is a node-based format. 

`template-kdl`, the format used by `bevy-kdl-scene` also supports defining
templates (or function) that can be used anywhere in your scene as a shortcut.

Please refer to the [kdl doc] for a quick overview of the kdl format, and refer
to the [template-kdl] doc for how templates are used.

A node named `entity` represents an entity. It has an **optional** argument,
either a number or a string. This argument is used to refer to the entity later
on.

The first node in the children of an entity is the list of its components, it
must be named `bundle`, and all its nodes are components. All other nodes are
the children entities of that entity. It is an error for the first node to not
be named `bundle`.

```kdl
scene {
  entity "player" {
    bundle {
      Player
      Hp 10
    }
  }
  entity "enemy1" {
    bundle {
      Enemy "Goblin"
      Hp 5
    }
  }
  entity "enemy2" {
    bundle {
      Enemy "Troll"
      Hp 20
    }
  }
  entity "enemy3" {
    bundle {
      Enemy "Kobold"
      Hp 15
    }
  }
}
```

At first, this might seem clunky and difficult to use. But it's without counting
on the power of [template-kdl]. You can define "prefabs" aka templates that can
take arguments and transform the final node:

```kdl
// This is a template (note that the `@` and `!` prefixes are purely
// optional, it's just easier to know what is an argument and what is
// a template this way)
!character "@id" "@hp" "@marker" {
  entity "@id" {
    bundle {
      @marker
      Hp "@hp"
    }
  }
}
!enemy "@id" "@type" "@hp" {
  !character "@id" "@hp" {
    Enemy "@type"
  }
}
scene {
  !character "player" 10 {
    Player
  }
  !enemy "enemy1" "Goblin" 5
  !enemy "enemy2" "Troll" 20
  !enemy "enemy3" "Kobold" 15
}
```

You'll notice that even if the initial format is clunky, it immediately becomes
a non-issue with templates. We encourage our users to rely heavily on templating
for their scene descriptions.

It is possible to create definition `kdl` files and export their templates to
re-use them in other files. Just name your final node `export` and provide
as argument the name of the templates you wish to export. To use a template
defined in another file, just prefix it with the name of the file it was
declared in. `template-kdl` does not support namespacing.

In `assets/base.kdl`:
```kdl
character "@id" "@hp" "@marker" {
  entity "@id" {
    bundle {
      @marker
      Hp "@hp"
    }
  }
}
enemy "@id" "@type" "@hp" {
  !character "@id" "@hp" {
    Enemy "@type"
  }
}
export "character" "enemy"
```

In `assets/room.kdl`:

```kdl
scene {
  "base.kdl/character" "player" 10 {
    Player
  }
  "base.kdl/enemy" "enemy1" "Goblin" 5
  "base.kdl/enemy" "enemy2" "Troll" 20
  "base.kdl/enemy" "enemy3" "Kobold" 15
}
```

## Marking

The scene loader, in addition to components, will also insert markers associating
the file and the position in which each component of an entity was declared. This
may be useful for error messages and diagnostics, if you are a plugin author
intending on depending on `bevy-kdl-scene`.

## Hot reloading

Hot reloading works as expected. In fact, far better than expected. The scene
loader adds markers to the entities it spawned, tracking which files it depends
on. When a `kdl` file is updated, the asset loader will look up which entities
have dependencies on this `kdl` file and reset its relevant component to their new
value. If the loader detects a change in entity hierarchy or new entities added to
the scene, it will remove and add back all sibling and children of the concerned
entities.



[kdl doc]: https://kdl.dev/
[template-kdl]: ./template-kdl/README.md
[bevy-reflect-deser]: ./bevy-reflect-deser/README.md
