use crate::error::{Error, Result};

use super::model::{Document, Element, Node, attribute_matches, attribute_text, string_at};

const MANIFEST_STRIP: [&str; 7] = [
    "split",
    "android:isFeatureSplit",
    "android:isSplitRequired",
    "android:requiredSplitTypes",
    "android:splitTypes",
    "android:configForSplit",
    "android:extractNativeLibs",
];

const APPLICATION_STRIP: [&str; 3] = [
    "android:isolatedSplits",
    "android:isSplitRequired",
    "android:extractNativeLibs",
];

const SPLIT_METADATA_PREFIXES: [&str; 1] = ["com.android.vending.splits"];

#[derive(Clone, Debug, Default)]
pub struct PatchReport {
    pub actions: Vec<String>,
}

pub fn make_standalone(document: &mut Document) -> Result<PatchReport> {
    let Document { strings, nodes, .. } = document;
    let names: &[String] = strings;

    let manifest = nodes
        .iter_mut()
        .find_map(|node| match node {
            Node::Element(element) if string_at(names, element.name) == Some("manifest") => {
                Some(element)
            }
            _ => None,
        })
        .ok_or_else(|| Error::Malformed("no <manifest> root element".to_owned()))?;

    let mut actions = Vec::new();
    strip_attributes(names, manifest, &MANIFEST_STRIP, &mut actions);
    strip_children(names, &mut manifest.children, &mut actions);

    Ok(PatchReport { actions })
}

fn strip_attributes(
    names: &[String],
    element: &mut Element,
    specs: &[&str],
    actions: &mut Vec<String>,
) {
    let label = string_at(names, element.name).unwrap_or("?").to_owned();
    element.attributes.retain(|attribute| {
        let hit = specs
            .iter()
            .any(|spec| attribute_matches(names, attribute, spec));
        if hit {
            let name = string_at(names, attribute.name).unwrap_or("?");
            actions.push(format!("removed attribute {name} from <{label}>"));
        }
        !hit
    });
}

fn strip_children(names: &[String], children: &mut Vec<Node>, actions: &mut Vec<String>) {
    children.retain(|node| match node {
        Node::Element(element) if is_split_metadata(names, element) => {
            actions.push("removed split metadata <meta-data> entry".to_owned());
            false
        }
        _ => true,
    });
    for node in children.iter_mut() {
        if let Node::Element(element) = node {
            let specs: &[&str] = if string_at(names, element.name) == Some("application") {
                &APPLICATION_STRIP
            } else {
                &[]
            };
            strip_attributes(names, element, specs, actions);
            strip_children(names, &mut element.children, actions);
        }
    }
}

fn is_split_metadata(names: &[String], element: &Element) -> bool {
    string_at(names, element.name) == Some("meta-data")
        && attribute_text(names, element, "android:name").is_some_and(|value| {
            SPLIT_METADATA_PREFIXES
                .iter()
                .any(|prefix| value.starts_with(*prefix))
        })
}
