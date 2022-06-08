# Template kdl

A serialization format piggy-backing on kdl, adding a
powerful yet minimalist template expension system. The API provides spans
to attribute nodes to their proper position in the source reader.

## Getting started

Add this to your `Cargo.toml`:
```toml
template-kdl = "0.3.0"
```

## Usage

A kdl file is a series of nodes. `template-kdl` will use read all but the last
node as template definitions. It is possible to export a template by finishing
the file with an `export` node. It is also possible to seed the templating
engine with your own templates programmatically.

Terms:
* `declaration site`: Where the template is defined
* `call site`: Where the template is used
* `body`: The node into which the template expands at call site, defined at
  declaration site.
* `targument` (actual parameter) is the value provided to the template as
  argument at call site.
* `tparameter` (formal parameter) is the name of variables to which a caller
  can assign arguments in a template.
  
I prefix those with `t` as to distinguish them from the kdl naming convention.

### Value templates

The simplest template has a single child node. All later nodes with the same
name will be expanded to the inner node.

```kdl, initial, 1-simple-replacement
my-favorite-washing-machine {
  WashingMachine noise_db=2.0 loading="Front" {
    Origin continent="Asia" country="China"
    Manifacturer brand="Miele" country="Germany"
  }
}
LastNodeInFile {
  my-favorite-washing-machine
  my-favorite-washing-machine
  my-favorite-washing-machine
}
```
becomes:
```kdl, target, 1-simple-replacement
LastNodeInFile {
  WashingMachine noise_db=2.0 loading="Front" {
    Origin continent="Asia" country="China"
    Manifacturer brand="Miele" country="Germany"
  }
  WashingMachine noise_db=2.0 loading="Front" {
    Origin continent="Asia" country="China"
    Manifacturer brand="Miele" country="Germany"
  }
  WashingMachine noise_db=2.0 loading="Front" {
    Origin continent="Asia" country="China"
    Manifacturer brand="Miele" country="Germany"
  }
}
```

Template nodes can refer to previously defined nodes, the reverse is false.

```kdl, initial, 2-early-bound-scoping
miele-data {
  Manifacturer brand="Miele" country="Germany"
}
my-favorite-washing-machine {
  WashingMachine noise_db=2.0 loading="Front" {
    origin-data // WRONG! origin_data is defined after this template
    miele-data
  }
}
origin-data {
  Origin continent="Asia" country="China"
}
LastNodeInFile {
  my-favorite-washing-machine
}
```
becomes
```kdl, target, 2-early-bound-scoping
LastNodeInFile {
  WashingMachine noise_db=2.0 loading="Front" {
    origin-data
    Manifacturer brand="Miele" country="Germany"
  }
}
```
Even if the template is generated after the declaration of `origin_data`.

The scope of the template is strictly defined by where it was declared.

## Function templates

A template can also be defined with a set of arguments with which to call it
when using it. For clarity, we refer to arguments at the definition site as
`tparameters`, and arguments at the calling site as `targuments`. This is to
distinguish them from the kdl's jargon. 

The simplest function template defines its `tparameters` as kdl arguments. To
use those templates, you must then provide values for declared `tparameters` at
the call site.

Note that you can also refer to `tparameters` by name in the call site.
```kdl, initial, 3-value-arg
my-favorite-washing-machine "noise" "loading-type" "country" {
  WashingMachine noise_db="noise" loading="loading-type" {
    Origin continent="Asia" country="country"
  }
}
LastNodeInFile {
  my-favorite-washing-machine 4.0 "Top" "Thailand" 
  // by name
  my-favorite-washing-machine loading-type="Back" noise=3.4 country="India"
}
```
becomes
```kdl, target, 3-value-arg
LastNodeInFile {
  WashingMachine noise_db=4.0 loading="Top" {
    Origin continent="Asia" country="Thailand"
  }
  WashingMachine noise_db=3.4 loading="Back" {
    Origin continent="Asia" country="India"
  }
}
```

All Kdl string values that exactly matches `tparameter` names will be
substitued with the `targument` provided at call site.

It is currently impossible to substitute the name of kdl parameters or name of
kdl nodes.

It is an error to call a template without the proper `targuments`, or with too
many `targuments`, or referencing `tparameter` names that do not exist.

You can also pass nodes as `targument`. To do so, simply use a node with the
the template name and the `targument` nodes as children:
```kdl, initial, 4-node-arg
my-favorite-washing-machine "manifacturer" {
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    manifacturer
  }
}
miele-data {
  Manifacturer brand="Miele" country="Germany"
}
LastNodeInFile {
  my-favorite-washing-machine {
    Manifacturer brand="GE" country="United States"
  }
  my-favorite-washing-machine {
    miele-data
  }
}
```
becomes
```kdl, target, 4-node-arg
LastNodeInFile {
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    Manifacturer brand="GE" country="United States"
  }
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    Manifacturer brand="Miele" country="Germany"
  }
}
```

It is an error to provide a `node` as `targuments` in a template where the
`tparameter` is used in value position, or providing a `value` where a `node`
is expected.


### Function templates with default targuments

It is possible to provide default `targuments` at the definition site. Those
`targuments` will be used if no other `targuments` is provided at the call site.

To provide default value `targuments`, simply use kdl parameters:

```kdl, initial, 5-value-arg-default
my-favorite-washing-machine noise=4.0 loading-type="Top" country="China" {
  WashingMachine noise_db="noise" loading="loading-type" {
    Origin continent="Asia" country="country"
  }
}
LastNodeInFile {
  my-favorite-washing-machine 5.0
  my-favorite-washing-machine country="Laos"
}
```
becomes
```kdl, target, 5-value-arg-default
LastNodeInFile {
  WashingMachine noise_db=5.0 loading="Top" {
    Origin continent="Asia" country="China"
  }
  WashingMachine noise_db=4.0 loading="Top" {
    Origin continent="Asia" country="Laos"
  }
}
```

It is also possible to define default node `targuments`. To do so, you need
to write them as children of the template node, just before the body node.

Note that `tparameters` expension only occurs in the **body** of the template,
hence, you cannot refer to the template's other `tparameters` in the default
node `targument` definition. However, expension of existing templates still
occur.

Default nodes are defined as the unique child of non-terminal children of the
template node.

It is currently impossible to refer to default node `tparameters` by name in
template call.

```kdl, initial, 6-node-arg-default
miele-details {
  Info weight=40.0 volume=4.0
}
my-favorite-washing-machine  {
  manifacturer {
    Manifacturer brand="Miele" country="Germany" { miele-details ; }
  }
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    manifacturer
  }
}
bosch-data {
  Manifacturer brand="Bosch" country="Germany"
}
LastNodeInFile {
  my-favorite-washing-machine {
    Manifacturer brand="GE" country="United States"
  }
  my-favorite-washing-machine { bosch-data ; }
  my-favorite-washing-machine
}
```
becomes
```kdl, target, 6-node-arg-default
LastNodeInFile {
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    Manifacturer brand="GE" country="United States"
  }
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    Manifacturer brand="Bosch" country="Germany"
  }
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    Manifacturer brand="Miele" country="Germany" {
      Info weight=40.0 volume=4.0
    }
  }
}
```

It is an error for direct children of a template that are not the body node to
have kdl entries.


### `expand` tparameters

A template node `tparameter` with the `expand` name acts in a special manner. A node
`tparameter` with the `expand` name must have a single argument and may have any
amount of children. When a node with the `expand` name and the provided argument is
encountered in the body of the template, the children node of the `targument` are
inserted into the encompassing document.

When called, the template node corresponding to the expand tparameter must be a node
with no entries. The children of the node will be expanded in the body.

```kdl, initial, 7-expand-arg
my-favorite-washing-machine  {
  // tparameters must be declared as `expand` this way
  expand "metadata"
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    // And they must be used in the body this way
    expand "metadata"
    Material drum="steel" shield="plastic"
  }
}
LastNodeInFile {
  my-favorite-washing-machine {
    // When calling a template with an expand tparmaeter, you must
    // provide its targument as a node.
    metadata {
      Manifacturer brand="Bosch" country="Germany"
      Info weight=40.0 volume=4.0
    }
  }
  my-favorite-washing-machine {
    metadata {
      Info weight=40.0 volume=4.0
    }
  }
}
```
becomes
```kdl, target, 7-expand-arg
LastNodeInFile {
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    // The node won't be expanded "as is" but inserted into the document
    Manifacturer brand="Bosch" country="Germany"
    Info weight=40.0 volume=4.0
    // Notice how the nodes after the insertion are preserved
    Material drum="steel" shield="plastic"
  }
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    Info weight=40.0 volume=4.0
    Material drum="steel" shield="plastic"
  }
}
```

It is also possible to provide default targuments to `expand` tparameters, just
specify the default list of node at declaration site. The list will be replaced
if the argument is specified.

If you want the default to be an empty list, **you must** declare it as so.

```kdl, initial, 8-expand-arg-default
my-favorite-washing-machine "detergent" {
  expand "metadata" {
      Manifacturer brand="Bosch" country="Germany"
      Info weight=40.0 volume=4.0
  }
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    detergent
    expand "metadata"
  }
}
laundry-detergent  {
  expand "ingredients" {
    Surfactant "alcohol ethoxylate"
    Builder "sodium carbonate"
    Bleach "sodium perborate"
  }
  Detergent {
    expand "ingredients"
  }
}
LastNodeInFile {
  my-favorite-washing-machine {
    laundry-detergent
  }
  my-favorite-washing-machine {
    laundry-detergent {
      ingredients {
        Surfactant "alkyl polyglycoside"
        Builder "polyphosphates"
        Bleach "tetraacetylethylenediamine"
        Enzyme "proteases"
      }
    }
    metadata {
      Info weight=40.0 volume=4.0
    }
  }
}
```
becomes
```kdl, target, 8-expand-arg-default
LastNodeInFile {
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    Detergent {
      Surfactant "alcohol ethoxylate"
      Builder "sodium carbonate"
      Bleach "sodium perborate"
    }
    Manifacturer brand="Bosch" country="Germany"
    Info weight=40.0 volume=4.0
  }
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"
    Detergent {
      Surfactant "alkyl polyglycoside"
      Builder "polyphosphates"
      Bleach "tetraacetylethylenediamine"
      Enzyme "proteases"
    }
    Info weight=40.0 volume=4.0
  }
}
```

### Rust API

TODO

