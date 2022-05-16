use std::str::FromStr;

use kdl::{KdlDocument, KdlEntry, KdlNode};
use miette::GraphicalReportHandler;
use pretty_assertions::assert_eq;
use template_kdl::read_document;

const README: &'static str = include_str!("../README.md");

#[derive(PartialEq, Debug, Clone, Copy)]
enum TemplateSide {
    Initial,
    Target,
}
impl FromStr for TemplateSide {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "initial" => Ok(Self::Initial),
            "target" => Ok(Self::Target),
            _ => Err(()),
        }
    }
}
struct KdlSection {
    content: &'static str,
    section: &'static str,
}

fn clear_fmt_recursive_node(node: &mut KdlNode) {
    node.clear_fmt();
    node.name_mut().clear_fmt();
    for entry in node.entries_mut().iter_mut() {
        entry.clear_fmt();
        entry.set_value_repr("");
        if let Some(name) = entry.name().cloned() {
            *entry = KdlEntry::new_prop(name, entry.value().clone());
        } else {
            *entry = KdlEntry::new(entry.value().clone());
        }
    }
    if let Some(doc) = node.children_mut() {
        clear_fmt_recursive_doc(doc);
    }
}
fn clear_fmt_recursive_doc(doc: &mut KdlDocument) {
    doc.clear_fmt();
    for node in doc.nodes_mut().iter_mut() {
        clear_fmt_recursive_node(node);
    }
}
/// Reads all fenced kdl code with a number from the README of this crate.
fn extract_kdls(side: TemplateSide) -> impl Iterator<Item = KdlSection> {
    README.split("\n```").filter_map(move |code_block| {
        let first_line = code_block.lines().next()?;
        let first_line_len = first_line.len();
        let mut metadata = first_line.split(", ");
        if metadata.next() != Some("kdl") {
            return None;
        }
        let kdl_side: TemplateSide = metadata.next()?.parse().ok()?;
        if side == kdl_side {
            let section = metadata.next()?;
            let content = code_block.get(first_line_len..)?;
            Some(KdlSection { content, section })
        } else {
            None
        }
    })
}

fn assert_eq_kdl(section_name: &str, target: &str, initial: &str) -> miette::Result<()> {
    println!("in section {section_name}");
    let actual: KdlDocument = initial.parse()?;
    let mut actual: KdlNode = read_document(&actual)
        .into_result()
        .unwrap()
        .evaluate()
        .into_result()
        .unwrap();
    let mut expected: KdlNode = target.parse()?;
    clear_fmt_recursive_node(&mut actual);
    clear_fmt_recursive_node(&mut expected);
    assert_eq!(actual.to_string(), expected.to_string());
    Ok(())
}

#[test]
fn readme_examples() -> miette::Result<()> {
    let mut initials: Vec<_> = extract_kdls(TemplateSide::Initial).collect();
    let mut targets: Vec<_> = extract_kdls(TemplateSide::Target).collect();
    initials.sort_unstable_by_key(|s| s.section);
    targets.sort_unstable_by_key(|s| s.section);
    for (initial, target) in initials.iter().zip(targets.iter()) {
        assert_eq!(initial.section, target.section);
        assert_eq_kdl(initial.section, target.content, initial.content)?;
    }
    Ok(())
}
