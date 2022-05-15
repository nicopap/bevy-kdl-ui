//! Deserialize a single rust data structure from a single KDL node.
//!
//! This includes proper error reporting and resilient traversal so
//! that it's possible to report more than a single error to the user.
use bevy_reflect::Reflect;

mod dyn_wrappers;
mod err;
mod newtype;
mod visit;

pub use err::{ConvertError, ConvertErrors, ConvertResult};
pub use visit::{convert_doc, from_doc};

pub type DynRefl = Box<dyn Reflect>;

#[cfg(test)]
#[allow(unused)]
mod test {
    use super::*;
    use bevy_reflect::{FromReflect, Reflect, TypeRegistration, TypeRegistry};
    use bevy_utils::HashMap;
    use kdl::KdlDocument;

    macro_rules! map {
        ($($key:expr => $value:expr),*$(,)?) => ({
            let mut ret = HashMap::default();
            $(ret.insert($key.to_owned(), $value);)*
            ret
        })
    }
    macro_rules! string_vec {
        ($($value:expr),*$(,)?) => (
            vec![$($value.to_owned(),)*]
        )
    }

    #[derive(Reflect, Debug, PartialEq, Default, FromReflect)]
    struct A {
        x: i32,
        d: D,
        c: C,
    }

    #[derive(Reflect, Debug, PartialEq, Clone, Copy, Default, FromReflect)]
    struct B;

    #[derive(Reflect, Debug, PartialEq, Default, FromReflect)]
    struct C(f32);

    #[derive(Clone, Reflect, Hash, PartialEq, Debug, Default, FromReflect)]
    #[reflect(Hash, PartialEq)]
    struct D {
        x: isize,
    }

    #[derive(Reflect, Copy, Clone, PartialEq, Debug)]
    #[reflect_value(PartialEq)]
    enum E {
        X,
        Y,
    }
    impl Default for E {
        fn default() -> Self {
            Self::X
        }
    }
    #[derive(FromReflect, PartialEq, Reflect, Default, Debug)]
    #[reflect(PartialEq)]
    struct F {
        b: Option<u8>,
        d: (i128, f32, String, f32, u32),
    }

    // TODO: Vec<complex struct>
    #[derive(PartialEq, Reflect, Default, Debug, FromReflect)]
    #[reflect(PartialEq)]
    struct G {
        y: Vec<String>,
        z: HashMap<String, f32>,
    }
    #[derive(PartialEq, Clone, Reflect, Default, Debug, FromReflect)]
    #[reflect(PartialEq)]
    struct Foo {
        xo: B,
        bar: i64,
        baz: String,
    }
    #[derive(Clone, PartialEq, Reflect, Default, Debug, FromReflect)]
    #[reflect(PartialEq)]
    struct Bar(f64);
    fn parse_kdl<T: FromReflect>(text: &str) -> ConvertResult<T> {
        let mut registry = TypeRegistry::default();
        macro_rules! register_all {
            ($($ty_name:ty ),* $(,)? ) => ({$(
                registry.register::<$ty_name>();
            )*})
        }
        macro_rules! register_more {
            ($($ty_name:ty ),* $(,)? ) => ({$(
                registry.add_registration(TypeRegistration::of::<$ty_name>());
            )*})
        }
        register_all!(
            Foo, Bar, A, B, C, D, E, F, G, bool, f64, f32, i8, i16, i32, i64, i128, isize, u8, u16,
            u32, u64, u128, usize, String,
        );
        register_more!((i128, f32, String, f32, u32), Option<u8>, Vec<String>, HashMap<String, f32>);
        let mut document: KdlDocument = text.parse().unwrap();
        convert_doc(&document, &registry).map(|val| T::from_reflect(val.as_ref()).unwrap())
    }
    #[test]
    fn test_component() {
        // for struct-type components
        // let kdl_foo = r#"Foo bar=1034 baz="hello" xo="B";"#;
        // let expected_foo = Foo { bar: 1034, baz: "hello".to_owned(), xo: B };
        // assert_eq!(parse_kdl::<Foo>(kdl_foo), Ok(expected_foo.clone()));

        // For tuple-type components
        let kdl_bar = r#"Bar 3.0;"#;
        let expected_bar = Bar(3.0);
        assert_eq!(parse_kdl::<Bar>(kdl_bar), Ok(expected_bar));
    }
    #[rustfmt::skip]
    #[test]
    fn more_test() {
        // TODO: Enum variants n' stuff
        // assert_eq!(parse_kdl::<E>("E \"Y\""), Ok(E::Y));

        assert_eq!(parse_kdl::<D>("D x=10;"), Ok(D { x: 10 }));
        assert_eq!(parse_kdl::<D>("D 10;"), Ok(D { x: 10 }));

        assert_eq!(parse_kdl::<C>("C 22.0;"), Ok(C(22.0)));

        assert_eq!(parse_kdl::<B>("B"), Ok(B));

        assert_eq!(
            // explicit declaration
            parse_kdl::<A>("A x=3030 { d x=140; c 444.0;}"),
            Ok(A { x: 3030, d: D { x: 140 }, c: C(444.0) })
        );
        assert_eq!(
            // Arbitrary order
            parse_kdl::<A>("A x=5151 { c 515.0; d 155; }"),
            Ok(A { x: 5151, d: D { x: 155 }, c: C(515.0) })
        );
        assert_eq!(
            // Anonymous declaration
            parse_kdl::<A>("A 4144 { D x=441; C 414.0;}"),
            Ok(A { x: 4144, d: D { x: 441 }, c: C(414.0) })
        );
        assert_eq!(
            // value type casting
            parse_kdl::<A>("A x=6161 c=616.0 d=16;"),
            Ok(A { x: 6161, d: D { x: 16 }, c: C(616.0) })
        );
        let f = r#"
        F {
           d -34234552 3943.13456 "I am a foo" 65431.25543243 0b101010101010101010101010;
           b 255;
        }"#;
        let f_v = F {
            d: (-34234552, 3943.13456, "I am a foo".to_owned(), 65431.25543243, 0b101010101010101010101010),
            b: Some(255),
        };
        assert_eq!(parse_kdl::<F>(f), Ok(f_v));
        let g = r#"
        G {
            y "hello" "this" "is" "a" "series" "of" "worlds";
            z pi=3.14 e=2.7182818 tau=6.28 ln2=0.69314; 
        } 
        "#;
        let g_v = G {
            y: string_vec!["hello", "this", "is", "a", "series", "of", "worlds"],
            z: map!{"pi" => 3.14, "e" => 2.7182818, "tau" => 6.28, "ln2" => 0.69314},
        };
        assert_eq!(parse_kdl::<G>(g), Ok(g_v));
    }
    // TODO: rewrite error tests
    // #[test]
    // fn parse_errors() {
    //     let ty_err = |exp: &str, act: &str| ConvertError::TypeMismatch {
    //         expected: exp.to_string(),
    //         actual: act.to_string(),
    //     };
    //     // Swap two anonymous declarations
    //     let doc = "A 1111 {  C 111.0; D 11;}";
    //     let err = parse_kdl::<A>(doc).unwrap_err();
    //     let mut err: Vec<_> = err.errors().map(|e| (&doc[e.range()], &e.error)).collect();
    //     err.sort_by_key(|t| t.0);
    //     assert_eq!(err[0], ("C", &ty_err("D", "C")));
    //     assert_eq!(err[1], ("D", &ty_err("C", "D")));
    //     assert_eq!(err.len(), 2);

    //     // Wrong type on newtype with field access
    //     let doc = "A x=2121 d=220.0 c=22;";
    //     let err = parse_kdl::<A>(doc).unwrap_err();
    //     let mut err: Vec<_> = err.errors().map(|e| (&doc[e.range()], &e.error)).collect();
    //     err.sort_by_key(|t| t.0);
    //     assert!(matches!(err[0], ("22", ConvertError::TypeMismatch { .. })));

    //     // wrong type in homogenous list and maps
    //     let doc = r#"
    //     G { y "hello" "this" 12 "a" "series" "of" "worlds";
    //         z pi=3.14 e=2.7182818 large=999999 tau=6.28 ln2=0.69314;  }"#;
    //     let err = parse_kdl::<G>(doc).unwrap_err();
    //     let mut err: Vec<_> = err.errors().collect();
    //     err.sort_by_key(|t| t.offset());
    //     let err: Vec<_> = err
    //         .into_iter()
    //         .map(|e| (&doc[e.range()], &e.error))
    //         .collect();
    //     assert_eq!(err[0], ("12", &ty_err("alloc::string::String", "int(12)")));
    //     assert_eq!(err[2], ("999999", &ty_err("f32", "int(999999)")));
    //     assert_eq!(err.len(), 5);
    // }
}
