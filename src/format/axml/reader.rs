use crate::error::{Error, Result};

use super::model::{
    Attribute, Document, Element, Node, TYPE_CDATA, TYPE_END_ELEMENT, TYPE_END_NAMESPACE,
    TYPE_RESOURCE_MAP, TYPE_START_ELEMENT, TYPE_START_NAMESPACE, TYPE_STRING_POOL, TYPE_XML,
    UTF8_FLAG, Value,
};

pub fn parse(data: &[u8]) -> Result<Document> {
    if u16_at(data, 0)? != TYPE_XML {
        return Err(malformed("missing XML root chunk"));
    }
    let header_size = usize::from(u16_at(data, 2)?);
    let total = (u32_at(data, 4)? as usize).min(data.len());
    if header_size < 8 || header_size > total {
        return Err(malformed("invalid XML root header"));
    }

    let mut document = Document::default();
    let mut root: Vec<Node> = Vec::new();
    let mut stack: Vec<Element> = Vec::new();
    let mut offset = header_size;

    while offset < total {
        let kind = u16_at(data, offset)?;
        let size = u32_at(data, offset + 4)? as usize;
        if size < 8 || offset + size > total {
            return Err(malformed(format!("bad chunk size {size} at {offset:#x}")));
        }
        let chunk = &data[offset..offset + size];
        match kind {
            TYPE_STRING_POOL => {
                let (strings, styles) = parse_string_pool(chunk)?;
                document.strings = strings;
                document.styles = styles;
            }
            TYPE_RESOURCE_MAP => {
                let count = (size - 8) / 4;
                document.resource_map = (0..count)
                    .map(|index| u32_at(chunk, 8 + index * 4))
                    .collect::<Result<Vec<u32>>>()?;
            }
            TYPE_START_NAMESPACE | TYPE_END_NAMESPACE => {
                let node = Node::Namespace {
                    start: kind == TYPE_START_NAMESPACE,
                    line: u32_at(chunk, 8)?,
                    comment: u32_at(chunk, 12)?,
                    prefix: u32_at(chunk, 16)?,
                    uri: u32_at(chunk, 20)?,
                };
                push_node(&mut root, &mut stack, node);
            }
            TYPE_START_ELEMENT => stack.push(parse_start_element(chunk)?),
            TYPE_END_ELEMENT => {
                let element = stack
                    .pop()
                    .ok_or_else(|| malformed("end element without matching start"))?;
                push_node(&mut root, &mut stack, Node::Element(element));
            }
            TYPE_CDATA => {
                let node = Node::Text {
                    line: u32_at(chunk, 8)?,
                    comment: u32_at(chunk, 12)?,
                    data: u32_at(chunk, 16)?,
                    value: read_value(chunk, 20)?,
                };
                push_node(&mut root, &mut stack, node);
            }
            other => return Err(malformed(format!("unsupported chunk type {other:#06x}"))),
        }
        offset += size;
    }

    if !stack.is_empty() {
        return Err(malformed("unclosed element at end of document"));
    }
    document.nodes = root;
    Ok(document)
}

fn push_node(root: &mut Vec<Node>, stack: &mut [Element], node: Node) {
    match stack.last_mut() {
        Some(parent) => parent.children.push(node),
        None => root.push(node),
    }
}

fn parse_start_element(chunk: &[u8]) -> Result<Element> {
    let attribute_start = usize::from(u16_at(chunk, 24)?);
    let attribute_size = usize::from(u16_at(chunk, 26)?);
    let count = usize::from(u16_at(chunk, 28)?);
    if attribute_start != 20 || attribute_size != 20 {
        return Err(malformed("unsupported attribute layout"));
    }
    let mut attributes = Vec::with_capacity(count);
    for index in 0..count {
        let base = 16 + attribute_start + index * attribute_size;
        attributes.push(Attribute {
            ns: u32_at(chunk, base)?,
            name: u32_at(chunk, base + 4)?,
            raw: u32_at(chunk, base + 8)?,
            value: read_value(chunk, base + 12)?,
        });
    }
    Ok(Element {
        line: u32_at(chunk, 8)?,
        comment: u32_at(chunk, 12)?,
        ns: u32_at(chunk, 16)?,
        name: u32_at(chunk, 20)?,
        attributes,
        id_index: u16_at(chunk, 30)?,
        class_index: u16_at(chunk, 32)?,
        style_index: u16_at(chunk, 34)?,
        children: Vec::new(),
    })
}

fn parse_string_pool(chunk: &[u8]) -> Result<(Vec<String>, Vec<Vec<u32>>)> {
    let string_count = u32_at(chunk, 8)? as usize;
    let style_count = u32_at(chunk, 12)? as usize;
    let flags = u32_at(chunk, 16)?;
    let strings_start = u32_at(chunk, 20)? as usize;
    let styles_start = u32_at(chunk, 24)? as usize;
    let utf8 = flags & UTF8_FLAG != 0;

    let mut strings = Vec::with_capacity(string_count);
    for index in 0..string_count {
        let relative = u32_at(chunk, 28 + index * 4)? as usize;
        let position = strings_start + relative;
        strings.push(if utf8 {
            read_utf8(chunk, position)?
        } else {
            read_utf16(chunk, position)?
        });
    }

    let style_offsets = 28 + string_count * 4;
    let mut styles = Vec::with_capacity(style_count);
    for index in 0..style_count {
        let relative = u32_at(chunk, style_offsets + index * 4)? as usize;
        let mut position = styles_start + relative;
        let mut words = Vec::new();
        loop {
            let word = u32_at(chunk, position)?;
            words.push(word);
            position += 4;
            if word == super::model::NO_INDEX {
                break;
            }
        }
        styles.push(words);
    }
    Ok((strings, styles))
}

fn read_utf16(chunk: &[u8], position: usize) -> Result<String> {
    let mut length = usize::from(u16_at(chunk, position)?);
    let mut cursor = position + 2;
    if length & 0x8000 != 0 {
        length = ((length & 0x7FFF) << 16) | usize::from(u16_at(chunk, cursor)?);
        cursor += 2;
    }
    let mut units = Vec::with_capacity(length);
    for index in 0..length {
        units.push(u16_at(chunk, cursor + index * 2)?);
    }
    String::from_utf16(&units).map_err(|_| malformed("invalid UTF-16 string in pool"))
}

fn read_utf8(chunk: &[u8], position: usize) -> Result<String> {
    let mut cursor = position;
    let _character_count = read_length8(chunk, &mut cursor)?;
    let byte_count = read_length8(chunk, &mut cursor)?;
    let bytes = chunk
        .get(cursor..cursor + byte_count)
        .ok_or_else(|| truncated(cursor))?;
    Ok(crate::util::decode_utf8_lenient(bytes))
}

fn read_length8(chunk: &[u8], cursor: &mut usize) -> Result<usize> {
    let first = usize::from(u8_at(chunk, *cursor)?);
    *cursor += 1;
    if first & 0x80 != 0 {
        let second = usize::from(u8_at(chunk, *cursor)?);
        *cursor += 1;
        Ok(((first & 0x7F) << 8) | second)
    } else {
        Ok(first)
    }
}

fn read_value(chunk: &[u8], offset: usize) -> Result<Value> {
    Ok(Value {
        size: u16_at(chunk, offset)?,
        res0: u8_at(chunk, offset + 2)?,
        data_type: u8_at(chunk, offset + 3)?,
        data: u32_at(chunk, offset + 4)?,
    })
}

fn malformed(message: impl Into<String>) -> Error {
    Error::Malformed(message.into())
}

fn truncated(offset: usize) -> Error {
    malformed(format!("data ends unexpectedly at offset {offset:#x}"))
}

fn u8_at(data: &[u8], offset: usize) -> Result<u8> {
    data.get(offset).copied().ok_or_else(|| truncated(offset))
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16> {
    match data.get(offset..offset + 2) {
        Some(bytes) => Ok(u16::from_le_bytes([bytes[0], bytes[1]])),
        None => Err(truncated(offset)),
    }
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    match data.get(offset..offset + 4) {
        Some(bytes) => Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
        None => Err(truncated(offset)),
    }
}
