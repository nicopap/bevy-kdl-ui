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

**WARNING**: the following code is not tested, it may or may not work as
announced.

The simplest template has a single child node. All later nodes with the same
name will be expanded to the inner node.

```kdl
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
```kdl
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

```kdl
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
```kdl
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
```kdl
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
```kdl
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
```kdl
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
```kdl
LastNodeInFile {
  WashingMachine noise_db=4.0 loading="Front" {
    Origin continent="Asia" country="China"par
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

```kdl
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
```kdl
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

```kdl
miele-details {
  Info weight=40.0 volume=4.0
}
my-favorite-washing-machine  {
  manifacturer {
    Manifacturer brand="Miele" country="Germany" { miele-details }
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
  my-favorite-washing-machine { bosh-data }
  my-favorite-washing-machine
}
```
becomes
```kdl
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


### `expand` special node

**WARNING**: not implemented yet.

In a template body, the special node with name `expand` can be used to flatten
a provided `targuments` into the children of a node in the body of the template.

the `expand` node specifically only allows a single argument. That argument must
be the name of a `tparameter`. The `targument` of that parameter must be a node.

### Rust API

TODO

