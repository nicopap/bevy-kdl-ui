use std::fmt;

use bevy_reflect::{FromReflect, Reflect, TypeRegistration, TypeRegistry};
use bevy_reflect_deser::{convert_doc, ConvertErrors};
use bevy_utils::HashMap;
use miette::GraphicalReportHandler;

macro_rules! string_vec {
    ($($value:expr),*$(,)?) => (
        vec![$($value.to_owned(),)*]
    )
}
macro_rules! map {
    ($($key:expr => $value:expr),*$(,)?) => ({
        let mut ret = HashMap::default();
        $(ret.insert($key.to_owned(), $value);)*
        ret
    })
}

#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct Coord(f64, f64);

#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct Foo {
    name: String,
    coordinates: Coord,
    populations: (u64, u32, u32),
    notable_place: String,
}
#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct Newtype(usize);
#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct NamedNewtype {
    inner: usize,
}
#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct NamedNestedNewtype {
    inner: Newtype,
}
type Example2 = (NamedNestedNewtype, NamedNewtype, Newtype);
#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct VecNewtype(Vec<String>);

#[derive(Reflect, Debug, FromReflect, PartialEq, Clone)]
struct SimpleFields {
    first_field: u8,
    second_field: String,
}
#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct CompoundFields {
    first: Vec<String>,
    second: SimpleFields,
    third: u8,
}
#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct NewtypeField {
    first_field: Newtype,
    second_field: String,
}
type MyTuple = (u8, SimpleFields, String);

#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct Fancy(String, u32);

const README: &'static str = include_str!("../README.md");

struct KdlSection {
    content: &'static str,
    section: u32,
}
fn extract_kdls() -> impl Iterator<Item = KdlSection> {
    README.split("\n```").filter_map(|section| {
        let first_line = section.lines().next()?;
        let first_line_len = first_line.len();
        let kdl_num: u32 = first_line.get(5..).and_then(|s| s.parse().ok())?;
        let content = section.get(first_line_len..)?;
        Some(KdlSection { section: kdl_num, content })
    })
}

fn assert_eq_kdl<T: FromReflect + PartialEq + fmt::Debug>(
    section_no: u32,
    text: &str,
    value: &T,
    reg: &TypeRegistry,
) -> Result<(), ConvertErrors> {
    println!("in section {section_no}");
    let converted = convert_doc(&text.parse().unwrap(), &reg)
        .map(|val| T::from_reflect(val.as_ref()).unwrap())?;
    assert_eq!(&converted, value);
    Ok(())
}

#[test]
fn readme_examples() {
    if let Err(err) = readme_examples_inner() {
        let mut output = String::new();
        GraphicalReportHandler::new()
            .render_report(&mut output, &err)
            .unwrap();
        println!("{output}");
        panic!()
    }
}

fn readme_examples_inner() -> Result<(), ConvertErrors> {
    let mut reg = TypeRegistry::default();
    macro_rules! register_all {
        ($($ty_name:ty ),* $(,)? ) => ({$(
            reg.register::<$ty_name>();
        )*})
    }
    macro_rules! register_more {
        ($($ty_name:ty ),* $(,)? ) => ({$(
            reg.add_registration(TypeRegistration::of::<$ty_name>());
        )*})
    }
    #[rustfmt::skip]
    register_all!(
        Coord, Foo, Newtype, NamedNewtype, NamedNestedNewtype, VecNewtype, SimpleFields,
        CompoundFields, NewtypeField, Fancy, u8, u32, usize, String, u64, f64
    );
    #[rustfmt::skip]
    register_more!(
        MyTuple, Example2, (u64, u32, u32), Vec<String>, Vec<u8>, Vec<Fancy>,
        HashMap<String, SimpleFields>, HashMap<String, u32>, Vec<usize>
    );

    let mut sections: Vec<_> = extract_kdls().collect();
    sections.sort_unstable_by_key(|s| s.section);

    let s1 = Foo {
        name: "西安".to_owned(),
        coordinates: Coord(108.95, 434.265),
        populations: (12953000, 429496, 1353000),
        notable_place: "Terracota army".to_owned(),
    };

    let s2 = (
        NamedNestedNewtype { inner: Newtype(1234) },
        NamedNewtype { inner: 2345 },
        Newtype(3456),
    );
    let s3 = VecNewtype(string_vec!["one", "two", "three", "four", "five"]);
    let s4_5_6 = SimpleFields {
        second_field: "Hello World".to_owned(),
        first_field: 34,
    };
    // TODO: 7
    let s8 = NamedNewtype { inner: 9999 };
    let s9_10_11 = CompoundFields {
        first: string_vec!["hello", "world"],
        second: s4_5_6.clone(),
        third: 3,
    };
    let s12 = NewtypeField {
        first_field: Newtype(9999),
        second_field: "Hello World".to_owned(),
    };
    let s13 = (25, s4_5_6.clone(), "Tuple String".to_owned());
    // TODO 14
    let s15 = Fancy("Hello".to_owned(), 9302);
    let s16 = vec![1usize, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let s17_18 = map! {"one" => 1u32, "two" => 2, "three" => 3, "four" => 4, "five" => 5};
    let s19 = vec![
        Fancy("One thousand".to_owned(), 1000),
        Fancy("Two thousand".to_owned(), 2000),
        Fancy("Three thousand".to_owned(), 3000),
        Fancy("Four thousand".to_owned(), 4000),
    ];
    let simple =
        |value, what: &str| SimpleFields { second_field: what.to_owned(), first_field: value };
    let s20 = map! {
        "ten" => simple(10, "Commandments"),
        "seven" => simple(7, "Dwarves"),
        "five" => simple(5, "Fingers"),
        "four" => simple(4, "Cardinal directions"),
    };
    let s21 = vec![1u8, 2, 3, 4];

    assert_eq_kdl(1, sections[0].content, &s1, &reg)?;
    assert_eq_kdl(2, sections[1].content, &s2, &reg)?;
    assert_eq_kdl(3, sections[2].content, &s3, &reg)?;
    assert_eq_kdl(4, sections[3].content, &s4_5_6, &reg)?;
    // assert_eq_kdl(5, sections[4].content, &s4_5_6, &reg)?;
    // assert_eq_kdl(6, sections[5].content, &s4_5_6, &reg)?;
    // assert_eq_kdl(7, sections[6].content, &s7, &reg)?;
    assert_eq_kdl(8, sections[7].content, &s8, &reg)?;
    assert_eq_kdl(9, sections[8].content, &s9_10_11, &reg)?;
    assert_eq_kdl(10, sections[9].content, &s9_10_11, &reg)?;
    assert_eq_kdl(11, sections[10].content, &s9_10_11, &reg)?;
    // assert_eq_kdl(12, sections[11].content, &s12, &reg)?;
    // assert_eq_kdl(13, sections[12].content, &s13, &reg)?;
    // assert_eq_kdl(14, sections[13].content, &s14, &reg)?;
    assert_eq_kdl(15, sections[14].content, &s15, &reg)?;
    assert_eq_kdl(16, sections[15].content, &s16, &reg)?;
    assert_eq_kdl(17, sections[16].content, &s17_18, &reg)?;
    // assert_eq_kdl(18, sections[17].content, &s17_18, &reg)?;
    assert_eq_kdl(19, sections[18].content, &s19, &reg)?;
    assert_eq_kdl(20, sections[19].content, &s20, &reg)?;
    assert_eq_kdl(21, sections[20].content, &s21, &reg)?;
    Ok(())
}
