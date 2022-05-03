use bevy_reflect::{NamedField, StructInfo};
use std::fmt;
use thiserror::Error;

pub mod owned_visit;
pub mod reflect;
pub mod visit;

pub struct RustFields(Vec<Field>);
impl fmt::Display for RustFields {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(Field { name, ty }) = self.0.first() {
            write!(f, "{name}: {ty}")?;
        }
        for Field { name, ty } in self.0.iter().skip(1) {
            write!(f, ", {name}: {ty}")?;
        }
        Ok(())
    }
}
pub struct Field {
    pub name: String,
    pub ty: String,
}
impl RustFields {
    pub fn from_info(info: &StructInfo) -> Self {
        let to_field = |f: &NamedField| Field {
            name: f.name().to_string(),
            ty: f.id().type_name().to_owned(),
        };
        Self(info.iter().map(to_field).collect())
    }
}

#[derive(Debug, Clone, Error)]
pub enum FieldIdentError {
    #[error("Invalid Identifier: `{0}`")]
    Invalid(String),
}
#[derive(Clone, Debug, Error)]
pub enum KdlDeserError {
    #[error(
        "You must use explicit field declaration with the `{type_name}` rust type, because \
it is a struct with named fields and has more than a single field.

Instead of `{type_name} {{\"value\"}} …` you should use the syntax `{type_name} {{.field=\"value\"}} …`"
    )]
    NotImplicit { type_name: String },
    #[error(
        "The KDL node for `{type_name}` is not compatible with its rust type:
The rust type has the fields:
    {missing}
While those fields are declared in the KDL file:
    {{actual}}"
    )]
    BadStructFields { type_name: String, missing: String },
    #[error("The type `{0}` is not registered.")]
    UnregisteredType(String),
    #[error("Implicit and explicit field declarations shouldn't be mixed.")]
    ImplicitMix,
    #[error("Invalid syntax for struct embedded into another one.")]
    InvalidFieldNode,
    #[error("There is no node to parse")]
    EmptyDocument,
    #[error("This error is for development purpose only")]
    Todo,
    #[error("Field identifier is erroneous: {0}")]
    Ident(#[from] FieldIdentError),
}
pub type KdlDResult<T> = Result<T, KdlDeserError>;
#[cfg(test)]
#[allow(unused)]
mod test {
    use super::*;
    use bevy_reflect::{Reflect, TypeRegistry};
    use kdl::KdlDocument;

    #[derive(Reflect, Debug, PartialEq, Default)]
    pub struct A {
        x: i32,
        d: D,
        c: C,
        // TODO: figure out how to do those
        // y: Vec<u32>,
        // z: HashMap<String, f32>,
    }

    #[derive(Reflect, Debug, PartialEq, Default)]
    pub struct B;

    #[derive(Reflect, Debug, PartialEq, Default)]
    pub struct C(usize);

    #[derive(Reflect, Hash, PartialEq, Debug, Default)]
    #[reflect(Hash, PartialEq)]
    pub struct D {
        x: isize,
    }

    #[derive(Reflect, Copy, Clone, PartialEq, Debug)]
    #[reflect_value(PartialEq)]
    pub enum E {
        X,
        Y,
    }
    impl Default for E {
        fn default() -> Self {
            Self::X
        }
    }

    #[derive(PartialEq, Clone, Reflect, Default, Debug)]
    #[reflect(PartialEq)]
    struct Foo {
        bar: i64,
        baz: String,
    }
    #[derive(Clone, PartialEq, Reflect, Default, Debug)]
    #[reflect(PartialEq)]
    struct Bar(f64);
    fn parse_kdl<T: Default + Reflect>(text: &str) -> T {
        let mut registry = TypeRegistry::default();
        registry.register::<Foo>();
        registry.register::<Bar>();
        registry.register::<A>();
        registry.register::<B>();
        registry.register::<C>();
        registry.register::<D>();
        registry.register::<E>();
        registry.register::<bool>();
        registry.register::<f64>();
        registry.register::<f32>();
        registry.register::<i8>();
        registry.register::<i16>();
        registry.register::<i32>();
        registry.register::<i64>();
        registry.register::<i128>();
        registry.register::<isize>();
        registry.register::<u8>();
        registry.register::<u16>();
        registry.register::<u32>();
        registry.register::<u64>();
        registry.register::<u128>();
        registry.register::<usize>();
        registry.register::<String>();
        let mut document: KdlDocument = text.parse().unwrap();
        let mut node = document.nodes_mut().pop().unwrap();
        let reflected = dbg!(owned_visit::parse_node(&mut node, &registry));
        let mut ret = T::default();
        ret.apply(reflected.unwrap().as_ref());
        ret
    }
    #[test]
    fn test_component() {
        // for struct-type components
        let kdl_foo = r#"Foo .bar=1034 .baz="hello";"#;
        let expected_foo = Foo {
            bar: 1034,
            baz: "hello".to_owned(),
        };
        assert_eq!(parse_kdl::<Foo>(kdl_foo), expected_foo);

        // For tuple-type components
        let kdl_bar = r#"Bar 3.0;"#;
        let expected_bar = Bar(3.0);
        assert_eq!(parse_kdl::<Bar>(kdl_bar), expected_bar);
    }
    #[test]
    fn more_test() {
        // TODO: in future rewrite, enable to level value types
        // const E_F: &str = r#"E "E::Y";"#;
        // assert_eq!(parse_kdl::<E>(E_F), E::Y);

        const D_F: &str = r#"D .x=10;"#;
        assert_eq!(parse_kdl::<D>(D_F), D { x: 10 });

        const C_F: &str = r#"C 22;"#;
        assert_eq!(parse_kdl::<C>(C_F), C(22));

        const B_F: &str = r#"B;"#;
        assert_eq!(parse_kdl::<B>(B_F), B);

        const A_F: &str = r#"A .x=3030 {
            .d "D" .x=143;
            .c "C" 444; 
        }"#;
        assert_eq!(
            parse_kdl::<A>(A_F),
            A {
                x: 3030,
                d: D { x: 143 },
                c: C(444)
            }
        );
    }
}
