use super::model::{
    Document, Node, TYPE_CDATA, TYPE_END_ELEMENT, TYPE_END_NAMESPACE, TYPE_RESOURCE_MAP,
    TYPE_START_ELEMENT, TYPE_START_NAMESPACE, TYPE_STRING_POOL, TYPE_XML, Value,
};

pub fn write(document: &Document) -> Vec<u8> {
    let mut body = string_pool(document);
    let mut ids = Vec::new();
    for id in &document.resource_map {
        put_u32(&mut ids, *id);
    }
    body.extend(chunk(TYPE_RESOURCE_MAP, 8, &ids));
    write_nodes(&mut body, &document.nodes);

    let mut out = Vec::with_capacity(body.len() + 8);
    put_u16(&mut out, TYPE_XML);
    put_u16(&mut out, 8);
    put_u32(&mut out, (body.len() + 8) as u32);
    out.extend(body);
    out
}

fn chunk(kind: u16, header_size: u16, rest: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + rest.len());
    put_u16(&mut out, kind);
    put_u16(&mut out, header_size);
    put_u32(&mut out, (8 + rest.len()) as u32);
    out.extend_from_slice(rest);
    out
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_value(out: &mut Vec<u8>, value: &Value) {
    put_u16(out, value.size);
    out.push(value.res0);
    out.push(value.data_type);
    put_u32(out, value.data);
}

fn string_pool(document: &Document) -> Vec<u8> {
    let mut string_offsets = Vec::new();
    let mut string_data = Vec::new();
    for text in &document.strings {
        put_u32(&mut string_offsets, string_data.len() as u32);
        let units: Vec<u16> = text.encode_utf16().collect();
        if units.len() < 0x8000 {
            put_u16(&mut string_data, units.len() as u16);
        } else {
            put_u16(&mut string_data, 0x8000 | ((units.len() >> 16) as u16));
            put_u16(&mut string_data, (units.len() & 0xFFFF) as u16);
        }
        for unit in units {
            put_u16(&mut string_data, unit);
        }
        put_u16(&mut string_data, 0);
    }
    while string_data.len() % 4 != 0 {
        string_data.push(0);
    }

    let mut style_offsets = Vec::new();
    let mut style_data = Vec::new();
    for style in &document.styles {
        put_u32(&mut style_offsets, style_data.len() as u32);
        for word in style {
            put_u32(&mut style_data, *word);
        }
    }

    let strings_start = (28 + string_offsets.len() + style_offsets.len()) as u32;
    let styles_start = if document.styles.is_empty() {
        0
    } else {
        strings_start + string_data.len() as u32
    };

    let mut rest = Vec::new();
    put_u32(&mut rest, document.strings.len() as u32);
    put_u32(&mut rest, document.styles.len() as u32);
    put_u32(&mut rest, 0);
    put_u32(&mut rest, strings_start);
    put_u32(&mut rest, styles_start);
    rest.extend(string_offsets);
    rest.extend(style_offsets);
    rest.extend(string_data);
    rest.extend(style_data);
    chunk(TYPE_STRING_POOL, 28, &rest)
}

fn write_nodes(out: &mut Vec<u8>, nodes: &[Node]) {
    for node in nodes {
        match node {
            Node::Namespace {
                start,
                line,
                comment,
                prefix,
                uri,
            } => {
                let kind = if *start {
                    TYPE_START_NAMESPACE
                } else {
                    TYPE_END_NAMESPACE
                };
                let mut rest = Vec::new();
                put_u32(&mut rest, *line);
                put_u32(&mut rest, *comment);
                put_u32(&mut rest, *prefix);
                put_u32(&mut rest, *uri);
                out.extend(chunk(kind, 16, &rest));
            }
            Node::Element(element) => {
                let mut rest = Vec::new();
                put_u32(&mut rest, element.line);
                put_u32(&mut rest, element.comment);
                put_u32(&mut rest, element.ns);
                put_u32(&mut rest, element.name);
                put_u16(&mut rest, 20);
                put_u16(&mut rest, 20);
                put_u16(&mut rest, element.attributes.len() as u16);
                put_u16(&mut rest, element.id_index);
                put_u16(&mut rest, element.class_index);
                put_u16(&mut rest, element.style_index);
                for attribute in &element.attributes {
                    put_u32(&mut rest, attribute.ns);
                    put_u32(&mut rest, attribute.name);
                    put_u32(&mut rest, attribute.raw);
                    put_value(&mut rest, &attribute.value);
                }
                out.extend(chunk(TYPE_START_ELEMENT, 16, &rest));

                write_nodes(out, &element.children);

                let mut end = Vec::new();
                put_u32(&mut end, element.line);
                put_u32(&mut end, element.comment);
                put_u32(&mut end, element.ns);
                put_u32(&mut end, element.name);
                out.extend(chunk(TYPE_END_ELEMENT, 16, &end));
            }
            Node::Text {
                line,
                comment,
                data,
                value,
            } => {
                let mut rest = Vec::new();
                put_u32(&mut rest, *line);
                put_u32(&mut rest, *comment);
                put_u32(&mut rest, *data);
                put_value(&mut rest, value);
                out.extend(chunk(TYPE_CDATA, 16, &rest));
            }
        }
    }
}
