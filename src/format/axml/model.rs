pub const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";
pub const NO_INDEX: u32 = 0xFFFF_FFFF;
pub const UTF8_FLAG: u32 = 1 << 8;

pub const TYPE_STRING_POOL: u16 = 0x0001;
pub const TYPE_XML: u16 = 0x0003;
pub const TYPE_START_NAMESPACE: u16 = 0x0100;
pub const TYPE_END_NAMESPACE: u16 = 0x0101;
pub const TYPE_START_ELEMENT: u16 = 0x0102;
pub const TYPE_END_ELEMENT: u16 = 0x0103;
pub const TYPE_CDATA: u16 = 0x0104;
pub const TYPE_RESOURCE_MAP: u16 = 0x0180;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Value {
    pub size: u16,
    pub res0: u8,
    pub data_type: u8,
    pub data: u32,
}

#[derive(Clone, Debug)]
pub struct Attribute {
    pub ns: u32,
    pub name: u32,
    pub raw: u32,
    pub value: Value,
}

#[derive(Clone, Debug)]
pub struct Element {
    pub line: u32,
    pub comment: u32,
    pub ns: u32,
    pub name: u32,
    pub attributes: Vec<Attribute>,
    pub id_index: u16,
    pub class_index: u16,
    pub style_index: u16,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug)]
pub enum Node {
    Namespace {
        start: bool,
        line: u32,
        comment: u32,
        prefix: u32,
        uri: u32,
    },
    Element(Element),
    Text {
        line: u32,
        comment: u32,
        data: u32,
        value: Value,
    },
}

#[derive(Clone, Debug, Default)]
pub struct Document {
    pub strings: Vec<String>,
    pub styles: Vec<Vec<u32>>,
    pub resource_map: Vec<u32>,
    pub nodes: Vec<Node>,
}

pub fn string_at(strings: &[String], index: u32) -> Option<&str> {
    if index == NO_INDEX {
        None
    } else {
        strings.get(index as usize).map(String::as_str)
    }
}

pub fn attribute_matches(strings: &[String], attribute: &Attribute, spec: &str) -> bool {
    match spec.strip_prefix("android:") {
        Some(name) => {
            string_at(strings, attribute.ns) == Some(ANDROID_NS)
                && string_at(strings, attribute.name) == Some(name)
        }
        None => attribute.ns == NO_INDEX && string_at(strings, attribute.name) == Some(spec),
    }
}

pub fn attribute_text(strings: &[String], element: &Element, spec: &str) -> Option<String> {
    let attribute = element
        .attributes
        .iter()
        .find(|attribute| attribute_matches(strings, attribute, spec))?;
    match string_at(strings, attribute.raw) {
        Some(text) => Some(text.to_owned()),
        None => Some(attribute.value.data.to_string()),
    }
}
