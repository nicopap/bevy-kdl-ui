use std::any::{type_name, Any};
use std::fmt;

use bevy_reflect::{FromReflect, Reflect, TypeRegistration, TypeRegistry, Typed};
use bevy_reflect_deser::{convert_doc, from_doc, ConvertErrors};
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
        $(ret.insert($key, $value);)*
        ret
    })
}

macro_rules! map_string {
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
struct ContainsMyTuple(MyTuple);

#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct Fancy(String, u32);

#[derive(Reflect, Debug, FromReflect, PartialEq)]
struct StringNewtype(String);

const README: &'static str = include_str!("../README.md");

struct KdlSection {
    content: &'static str,
    section: u32,
}
/// Reads all fenced kdl code with a number from the README of this crate.
fn extract_kdls() -> impl Iterator<Item = KdlSection> {
    README.split("\n```").filter_map(|section| {
        let first_line = section.lines().next()?;
        let first_line_len = first_line.len() + 1;
        let content = section.get(first_line_len..)?;
        let kdl_num = first_line.get(5..).and_then(|s| s.parse().ok())?;
        first_line
            .starts_with("kdl, ")
            .then(|| KdlSection { section: kdl_num, content })
    })
}

fn assert_all_lines_eq_kdl<T: FromReflect + PartialEq + fmt::Debug + Typed>(
    section_no: u32,
    text: &str,
    value: &T,
    reg: &TypeRegistry,
) -> Result<(), ConvertErrors> {
    println!("in section {section_no}");
    for (i, line) in text.lines().enumerate() {
        println!("########### line {i} ###############\n---------------------");
        let converted = from_doc::<T>(line.parse().unwrap(), &reg)
            .map(|val| T::from_reflect(val.as_ref()).unwrap())?;
        assert_eq!(&converted, value, "in {line}");
    }
    Ok(())
}

fn assert_eq_kdl<T: FromReflect + PartialEq + fmt::Debug>(
    section_no: u32,
    text: &str,
    value: &T,
    reg: &TypeRegistry,
) -> Result<(), ConvertErrors> {
    println!("in section {section_no}");
    let converted = convert_doc(text.parse().unwrap(), &reg)
        .map(|val| T::from_reflect(val.as_ref()).unwrap())?;
    assert_eq!(&converted, value, "in {text}");
    Ok(())
}
fn assert_fails_kdl<T: FromReflect + fmt::Debug + Any>(
    section_no: u32,
    text: &str,
    reg: &TypeRegistry,
) -> Result<(), ConvertErrors> {
    println!("in section {section_no}");
    let converted =
        convert_doc(text.parse().unwrap(), &reg).map(|val| T::from_reflect(val.as_ref()));

    assert!(
        converted.is_err(),
        "{converted:?}: {} in\n{text}",
        type_name::<T>()
    );
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
        CompoundFields, NewtypeField, Fancy, u8, u32, usize, String, u64, f64,
        ContainsMyTuple, StringNewtype
    );
    #[rustfmt::skip]
    register_more!(
        MyTuple, Example2, (u64, u32, u32), Vec<String>, Vec<u8>, Vec<Fancy>,
        HashMap<String, SimpleFields>, HashMap<String, u32>, Vec<usize>,
        HashMap<u32, Fancy>, HashMap<u32, StringNewtype>
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
    let s3_26 = VecNewtype(string_vec!["one", "two", "three", "four", "five"]);
    let s4_5_7 = SimpleFields {
        second_field: "Hello World".to_owned(),
        first_field: 34,
    };
    let s8 = NamedNewtype { inner: 9999 };
    let s9_10_11 = CompoundFields {
        first: string_vec!["hello", "world"],
        second: s4_5_7.clone(),
        third: 3,
    };
    let s12 = NewtypeField {
        first_field: Newtype(9999),
        second_field: "Hello World".to_owned(),
    };
    let s13_29 = ContainsMyTuple((25, s4_5_7.clone(), "Tuple String".to_owned()));
    let s15 = Fancy("Hello".to_owned(), 9302);
    let s16 = vec![1usize, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let s17 = map_string! {"one" => 1u32, "two" => 2, "three" => 3, "four" => 4, "five" => 5};
    let fancy = |s: &str, i| Fancy(s.to_owned(), i);
    let s18: HashMap<u32, Fancy> = map! {
        1 => fancy("Hello world", 1),
        2 => fancy("Bonjour le monde", 2),
        3 => fancy("Hallo Welt", 3),
        4 => fancy("Ahoj svĕte", 4),
    };
    let s19 = vec![
        Fancy("One thousand".to_owned(), 1000),
        Fancy("Two thousand".to_owned(), 2000),
        Fancy("Three thousand".to_owned(), 3000),
        Fancy("Four thousand".to_owned(), 4000),
    ];
    let simple =
        |value, what: &str| SimpleFields { second_field: what.to_owned(), first_field: value };
    let s20 = map_string! {
        "ten" => simple(10, "Commandments"),
        "seven" => simple(7, "Dwarves"),
        "five" => simple(5, "Fingers"),
        "four" => simple(4, "Cardinal directions"),
    };
    let s21 = vec![1u8, 2, 3, 4];
    let s27 = NamedNestedNewtype { inner: Newtype(9999) };
    let s30: HashMap<u32, _> = map! {
        1 => StringNewtype("Hello world".to_owned()),
        2 => StringNewtype("Bonjour le monde".to_owned()),
        3 => StringNewtype("Hallo Welt".to_owned()),
        4 => StringNewtype("Ahoj svĕte".to_owned()),
    };

    assert_eq_kdl(1, sections[0].content, &s1, &reg)?;
    assert_eq_kdl(2, sections[1].content, &s2, &reg)?;
    assert_eq_kdl(3, sections[2].content, &s3_26, &reg)?;
    assert_eq_kdl(4, sections[3].content, &s4_5_7, &reg)?;
    assert_eq_kdl(5, sections[4].content, &s4_5_7, &reg)?;
    assert_fails_kdl::<SimpleFields>(6, sections[5].content, &reg)?;
    assert_eq_kdl(7, sections[6].content, &s4_5_7, &reg)?;
    assert_eq_kdl(8, sections[7].content, &s8, &reg)?;
    assert_eq_kdl(9, sections[8].content, &s9_10_11, &reg)?;
    assert_eq_kdl(10, sections[9].content, &s9_10_11, &reg)?;
    assert_eq_kdl(11, sections[10].content, &s9_10_11, &reg)?;
    assert_eq_kdl(12, sections[11].content, &s12, &reg)?;
    assert_eq_kdl(13, sections[12].content, &s13_29, &reg)?;
    assert_fails_kdl::<MyTuple>(14, sections[13].content, &reg)?;
    assert_eq_kdl(15, sections[14].content, &s15, &reg)?;
    assert_eq_kdl(16, sections[15].content, &s16, &reg)?;
    assert_eq_kdl(17, sections[16].content, &s17, &reg)?;
    assert_eq_kdl(18, sections[17].content, &s18, &reg)?;
    assert_eq_kdl(19, sections[18].content, &s19, &reg)?;
    assert_eq_kdl(20, sections[19].content, &s20, &reg)?;
    assert_eq_kdl(21, sections[20].content, &s21, &reg)?;
    assert_fails_kdl::<SimpleFields>(22, sections[21].content, &reg)?;
    assert_fails_kdl::<NamedNewtype>(23, sections[22].content, &reg)?;
    assert_fails_kdl::<SimpleFields>(24, sections[23].content, &reg)?;
    assert_fails_kdl::<NamedNewtype>(25, sections[24].content, &reg)?;
    assert_eq_kdl(26, sections[25].content, &s3_26, &reg)?;
    assert_all_lines_eq_kdl(27, sections[26].content, &s27, &reg)?;
    assert_fails_kdl::<MyTuple>(28, sections[27].content, &reg)?;
    assert_eq_kdl(29, sections[28].content, &s13_29.0, &reg)?;
    assert_eq_kdl(30, sections[29].content, &s30, &reg)?;
    Ok(())
}
