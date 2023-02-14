## Complex keys in maps

It is currently impossible to create `Map` types with complex data structures
as key type because `Dynamic*` (which is the underlying type representing
complex data structures in `bevy-reflect-deser`) always return `None` on
`Reflect::reflect_hash`. It seems possible to "sidecast" the underlying type
by adding a `TypeRegistration::sidecast_into` method to all `ReflectFrom`
type registrations. But it is currently simply impossible to infere a proper
hash for those. So I'll keep the example for HashMap with complex keys in here

```kdl, 18
"HashMap<Fancy, String>" {
  - {
    Fancy "Hello world" 1
    String "English"
  }
  - {
    Fancy "Bonjour le monde" 2
    String "Français"
  }
  - {
    Fancy "Hallo Welt" 3
    String "Deutsch"
  }
  - {
    Fancy "Ahoj svĕte" 4
    String "Čeština"
  }
}
```


