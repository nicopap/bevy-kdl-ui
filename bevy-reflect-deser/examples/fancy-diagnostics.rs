use bevy_reflect::{FromReflect, Reflect, TypeRegistration, TypeRegistry};
use bevy_reflect_deser::*;
use bevy_utils::HashMap;
use miette::Result;

#[derive(Reflect, Debug, FromReflect)]
struct Marker;

#[derive(Reflect, FromReflect, Debug)]
struct Newtype(u32);

#[derive(Reflect, FromReflect, Debug)]
struct VeryNewtype {
    inner: Newtype,
}

#[derive(Reflect, Debug, FromReflect)]
struct NumberContainer {
    u8: u8,
    u32: u32,
    usize: usize,
    f64: f64,
    f32: f32,
    opt_f32: Option<f32>,
    i32: i32,
    opt_i128: Option<i128>,
}

#[derive(Reflect, FromReflect, Debug)]
struct Regular {
    weight: f32,
    name: String,
}

#[derive(Reflect, Debug, FromReflect)]
struct NewtypeContainer(HashMap<String, VeryNewtype>);

#[derive(Reflect, FromReflect, Debug)]
struct Bar {
    name: String,
    waiter_names: Vec<String>,
    regulars: HashMap<String, Regular>,
    newbies: Vec<VeryNewtype>,
}

#[derive(Reflect, Debug, FromReflect)]
struct Coord(f64, f64);
#[derive(Reflect, Debug, FromReflect)]
struct Foo {
    name: String,
    coordinates: Coord,
    populations: (u64, u32, u32),
    notable_place: String,
}

const KDL_DEFS: &[&str] = &[
    r#"Marker"#,
    r#"Newtype 9000"#,
    r#"VeryNewtype inner=9001"#,
    r#"NumberContainer  u8=255  i32=-342334455  u32=4294967295   f32=31.3131  f64=-3.14e-5  usize=4294967295   opt_f32=3.4323   opt_i128=-103444434 "#,
    r#"Miguel { 
         mig  weight=83.9  name="Miguel Enríquez"
    }
    Bar  name="The jolly roger" {
         waiter_names "Bill Bones" "Toothless Pete" "Musclemouth Mike" "Thunder Dave"
         regulars {
             pierre  weight=90.0  name="Pierre Lafitte"
             sam  weight=83.1  name="Sam Hall Lord"
            // Every nodes declared before the last one act as a template
            // that expand to their content
            Miguel
             fran  weight=79.9  name="Francisco de Miranda"
        }
        // Notice how our ints are implicitly converted to VeryNewtype
         newbies 1 2 3 4 5 6
    }"#,
    r#"Foo  name="西安" {
        // Tuples and tuple structs are anonymous
         coordinates  34.265  108.954
         populations 12953000 429496 1353000
         notable_place "Terracota army" 
    }"#,
    // Auto-unwrapping of newtypes is especially useful when you have a list of newtypes
    // Note that it is currently necessary to specify the field as `.0`
    // for more complex inner types.
    r#"NewtypeContainer nine=9 eight=8 seven=7 six=6 five=5 four=4 three=3 two=2 one=1"#,
];
fn main() -> Result<()> {
    macro_rules! reg_with {
        ($($ty_name:ty),* $(,)?) => ({
            let mut reg = TypeRegistry::default();
            $( reg.add_registration(TypeRegistration::of::<$ty_name>()); )*
            reg
        });
    }
    #[rustfmt::skip]
    let reg = reg_with!(
        Marker, Newtype, VeryNewtype, NumberContainer, Regular, NewtypeContainer,
        Bar, Coord, Foo, (u64, u32, u32), String, u8, i32, u32, f32, f64, usize,
        Option<f32>, Option<i128>, HashMap<String, Regular>, Vec<VeryNewtype>, 
        Vec<String>, u64, HashMap<String, VeryNewtype>,
    );
    for def in KDL_DEFS {
        let doc = def.parse().unwrap();
        let reflect = convert_doc(&doc, &reg)?;
        if let Some(m) = Newtype::from_reflect(reflect.as_ref()) {
            println!("Netype: {m:?}");
        } else if let Some(m) = NumberContainer::from_reflect(reflect.as_ref()) {
            println!("NumberContainer: {m:#?}");
        } else if let Some(m) = Bar::from_reflect(reflect.as_ref()) {
            println!("Bar: {m:#?}");
        } else if let Some(m) = Foo::from_reflect(reflect.as_ref()) {
            println!("Foo: {m:#?}");
        } else if let Some(m) = NewtypeContainer::from_reflect(reflect.as_ref()) {
            println!("NewtypeContainer: {m:#?}");
        } else if let Some(m) = Marker::from_reflect(reflect.as_ref()) {
            println!("Marker: {m:?}");
        } else {
            println!("None recognized: {reflect:?}");
        }
    }
    Ok(())
}
