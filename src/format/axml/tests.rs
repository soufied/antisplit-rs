use super::model::{ANDROID_NS, Attribute, Document, Element, NO_INDEX, Node, Value};
use super::{make_standalone, parse, write};

fn text(index: u32) -> Value {
    Value {
        size: 8,
        res0: 0,
        data_type: 0x03,
        data: index,
    }
}

fn number(value: u32) -> Value {
    Value {
        size: 8,
        res0: 0,
        data_type: 0x10,
        data: value,
    }
}

fn flag(value: bool) -> Value {
    Value {
        size: 8,
        res0: 0,
        data_type: 0x12,
        data: u32::from(value),
    }
}

fn attribute(ns: u32, name: u32, raw: u32, value: Value) -> Attribute {
    Attribute {
        ns,
        name,
        raw,
        value,
    }
}

fn element(name: u32, attributes: Vec<Attribute>, children: Vec<Node>) -> Node {
    Node::Element(Element {
        line: 3,
        comment: NO_INDEX,
        ns: NO_INDEX,
        name,
        attributes,
        id_index: 0,
        class_index: 0,
        style_index: 0,
        children,
    })
}

fn sample() -> Document {
    let strings: Vec<String> = [
        "manifest",
        "package",
        "split",
        ANDROID_NS,
        "isSplitRequired",
        "versionCode",
        "application",
        "meta-data",
        "name",
        "com.android.vending.splits.required",
        "isolatedSplits",
        "com.example.app",
        "android",
    ]
    .iter()
    .map(|text| (*text).to_owned())
    .collect();

    let split_meta = element(7, vec![attribute(3, 8, 9, text(9))], Vec::new());
    let kept_meta = element(7, vec![attribute(3, 8, 11, text(11))], Vec::new());
    let application = element(
        6,
        vec![attribute(3, 10, NO_INDEX, flag(true))],
        vec![split_meta, kept_meta],
    );
    let manifest = element(
        0,
        vec![
            attribute(NO_INDEX, 1, 11, text(11)),
            attribute(NO_INDEX, 2, 11, text(11)),
            attribute(3, 4, NO_INDEX, flag(true)),
            attribute(3, 5, NO_INDEX, number(42)),
        ],
        vec![application],
    );

    Document {
        strings,
        styles: Vec::new(),
        resource_map: vec![0x0101_021b, 0x0101_0280, 0x0101_0281],
        nodes: vec![
            Node::Namespace {
                start: true,
                line: 1,
                comment: NO_INDEX,
                prefix: 12,
                uri: 3,
            },
            manifest,
            Node::Namespace {
                start: false,
                line: 1,
                comment: NO_INDEX,
                prefix: 12,
                uri: 3,
            },
        ],
    }
}

#[test]
fn round_trip_is_stable() {
    let original = sample();
    let first = write(&original);
    let parsed = parse(&first).expect("parse");
    let second = write(&parsed);
    assert_eq!(first, second);
    assert_eq!(parsed.strings, original.strings);
    assert_eq!(parsed.resource_map, original.resource_map);
}

#[test]
fn standalone_patch_strips_split_markers() {
    let mut document = parse(&write(&sample())).expect("parse");
    let report = make_standalone(&mut document).expect("patch");
    assert_eq!(report.actions.len(), 4);

    let bytes = write(&document);
    let mut again = parse(&bytes).expect("reparse");
    let recheck = make_standalone(&mut again).expect("recheck");
    assert!(recheck.actions.is_empty());
}
