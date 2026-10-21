use crate::error::{Error, Result};

pub const RES_STRING_POOL: u16 = 0x0001;
pub const RES_TABLE: u16 = 0x0002;
pub const RES_TABLE_PACKAGE: u16 = 0x0200;
pub const RES_TABLE_TYPE: u16 = 0x0201;
pub const RES_TABLE_TYPE_SPEC: u16 = 0x0202;
pub const NO_ENTRY: u32 = 0xFFFF_FFFF;
pub const NO_INDEX: u32 = 0xFFFF_FFFF;
pub const DATA_TYPE_STRING: u8 = 0x03;
pub const FLAG_COMPLEX: u16 = 0x0001;
const FLAG_COMPACT: u16 = 0x0008;
const UTF8_FLAG: u32 = 1 << 8;
const SORTED_FLAG: u32 = 1;
const TYPE_FLAG_SPARSE: u8 = 0x01;
const TYPE_FLAG_OFFSET16: u8 = 0x02;
const TABLE_HEADER: usize = 12;
const POOL_HEADER: usize = 28;
const PACKAGE_HEADER: usize = 288;
const TYPE_HEADER: usize = 20;
const TYPE_SPEC_HEADER: usize = 16;
const NAME_UNITS: usize = 128;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StringPool {
    pub strings: Vec<String>,
    pub styles: Vec<Vec<u32>>,
    pub utf8: bool,
    pub sorted: bool,
}

impl StringPool {
    pub fn from_strings(strings: Vec<String>, utf8: bool) -> Self {
        Self {
            strings,
            styles: Vec::new(),
            utf8,
            sorted: false,
        }
    }

    pub fn style_of(&self, index: usize) -> Vec<u32> {
        match self.styles.get(index) {
            Some(words) if !matches!(words.as_slice(), [NO_INDEX]) => words.clone(),
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResConfig {
    pub raw: Vec<u8>,
}

impl ResConfig {
    pub fn read(data: &[u8], offset: usize) -> Result<Self> {
        let size = u32_at(data, offset)? as usize;
        if size < 4 {
            return Err(malformed("resource config is smaller than its size field"));
        }
        Ok(Self {
            raw: slice(data, offset, size)?.to_vec(),
        })
    }
}

#[cfg(test)]
impl ResConfig {
    pub fn zeroed() -> Self {
        const CONFIG_SIZE: usize = 52;
        let mut raw = vec![0u8; CONFIG_SIZE];
        raw[..4].copy_from_slice(&(CONFIG_SIZE as u32).to_le_bytes());
        Self { raw }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResValue {
    pub size: u16,
    pub res0: u8,
    pub data_type: u8,
    pub data: u32,
}

impl ResValue {
    pub fn new(data_type: u8, data: u32) -> Self {
        Self {
            size: 8,
            res0: 0,
            data_type,
            data,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapItem {
    pub name: u32,
    pub value: ResValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryValue {
    Simple(ResValue),
    Map { parent: u32, items: Vec<MapItem> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub flags: u16,
    pub key: u32,
    pub value: EntryValue,
    pub extra: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeBlock {
    pub id: u8,
    pub reserved: u16,
    pub config: ResConfig,
    pub entries: Vec<Option<Entry>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeSpec {
    pub id: u8,
    pub res0: u8,
    pub res1: u16,
    pub flags: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Child {
    TypeSpec(TypeSpec),
    Type(TypeBlock),
    Raw(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub id: u32,
    pub name: String,
    pub last_public_type: u32,
    pub last_public_key: u32,
    pub type_id_offset: u32,
    pub header_extra: Vec<u8>,
    pub type_strings: StringPool,
    pub key_strings: StringPool,
    pub children: Vec<Child>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub global: StringPool,
    pub packages: Vec<Package>,
}

pub fn parse(data: &[u8]) -> Result<Table> {
    if u16_at(data, 0)? != RES_TABLE {
        return Err(malformed("missing resource table header"));
    }
    let header_size = usize::from(u16_at(data, 2)?);
    let size = chunk_size(data, 0)?;
    if header_size < TABLE_HEADER || header_size > size {
        return Err(malformed("invalid resource table header size"));
    }
    let table = &data[..size];
    let package_count = u32_at(table, 8)? as usize;
    let global = read_pool(table, header_size)?;
    let mut position = header_size + chunk_size(table, header_size)?;
    let mut packages = Vec::with_capacity(package_count.min(64));
    for _ in 0..package_count {
        if u16_at(table, position)? != RES_TABLE_PACKAGE {
            return Err(Error::Unsupported(
                "unexpected top-level chunk in the resource table".to_owned(),
            ));
        }
        let length = chunk_size(table, position)?;
        packages.push(parse_package(table, position)?);
        position += length;
    }
    if position != size {
        return Err(malformed("trailing data after the resource packages"));
    }
    Ok(Table { global, packages })
}

pub fn serialize(table: &Table) -> Vec<u8> {
    let global = encode_pool(&table.global);
    let packages: Vec<Vec<u8>> = table.packages.iter().map(encode_package).collect();
    let body: usize = global.len() + packages.iter().map(Vec::len).sum::<usize>();
    let mut rest = Vec::with_capacity(4 + body);
    put_u32(&mut rest, table.packages.len() as u32);
    rest.extend(global);
    for package in packages {
        rest.extend(package);
    }
    chunk(RES_TABLE, TABLE_HEADER as u16, &rest)
}

fn parse_package(data: &[u8], start: usize) -> Result<Package> {
    let header_size = usize::from(u16_at(data, start + 2)?);
    let size = chunk_size(data, start)?;
    if header_size < 12 || header_size > size {
        return Err(malformed("invalid package header size"));
    }
    let body_end = start + size;
    let id = u32_at(data, start + 8)?;
    let name = decode_name(slice(data, start + 12, NAME_UNITS * 2)?);
    let type_offset = header_u32(data, start, header_size, 268)? as usize;
    let last_public_type = header_u32(data, start, header_size, 272)?;
    let key_offset = header_u32(data, start, header_size, 276)? as usize;
    let last_public_key = header_u32(data, start, header_size, 280)?;
    let type_id_offset = header_u32(data, start, header_size, 284)?;
    let header_extra = if header_size > PACKAGE_HEADER {
        slice(data, start + PACKAGE_HEADER, header_size - PACKAGE_HEADER)?.to_vec()
    } else {
        Vec::new()
    };
    let type_pool_position = start + type_offset;
    let key_pool_position = start + key_offset;
    let type_strings = read_pool(data, type_pool_position)?;
    let key_strings = read_pool(data, key_pool_position)?;

    let mut children = Vec::new();
    let mut position = start + header_size;
    while position < body_end {
        let kind = u16_at(data, position)?;
        let length = chunk_size(data, position)?;
        if position + length > body_end {
            return Err(malformed("package child chunk overruns its package"));
        }
        if position == type_pool_position || position == key_pool_position {
            position += length;
            continue;
        }
        let child = match kind {
            RES_TABLE_TYPE_SPEC => Child::TypeSpec(parse_type_spec(data, position)?),
            RES_TABLE_TYPE => Child::Type(parse_type(data, position)?),
            _ => Child::Raw(slice(data, position, length)?.to_vec()),
        };
        children.push(child);
        position += length;
    }

    Ok(Package {
        id,
        name,
        last_public_type,
        last_public_key,
        type_id_offset,
        header_extra,
        type_strings,
        key_strings,
        children,
    })
}

fn parse_type_spec(data: &[u8], position: usize) -> Result<TypeSpec> {
    let header_size = usize::from(u16_at(data, position + 2)?);
    let id = u8_at(data, position + 8)?;
    let res0 = u8_at(data, position + 9)?;
    let res1 = u16_at(data, position + 10)?;
    let count = u32_at(data, position + 12)? as usize;
    let flags_start = position + header_size;
    let flags = (0..count)
        .map(|index| u32_at(data, flags_start + index * 4))
        .collect::<Result<Vec<u32>>>()?;
    Ok(TypeSpec {
        id,
        res0,
        res1,
        flags,
    })
}

fn parse_type(data: &[u8], position: usize) -> Result<TypeBlock> {
    let header_size = usize::from(u16_at(data, position + 2)?);
    let id = u8_at(data, position + 8)?;
    let flags = u8_at(data, position + 9)?;
    let reserved = u16_at(data, position + 10)?;
    let count = u32_at(data, position + 12)? as usize;
    let entries_start = u32_at(data, position + 16)? as usize;
    let config = ResConfig::read(data, position + 20)?;
    let chunk_end = position + chunk_size(data, position)?;
    let offsets_start = position + header_size;
    let entries_base = position + entries_start;
    let mut entries: Vec<Option<Entry>> = vec![None; count];

    if flags & TYPE_FLAG_SPARSE != 0 {
        let mut pairs = Vec::with_capacity(count);
        for index in 0..count {
            let base = offsets_start + index * 4;
            let entry_id = usize::from(u16_at(data, base)?);
            let offset = usize::from(u16_at(data, base + 2)?) * 4;
            pairs.push((entry_id, offset));
        }
        let length = pairs.iter().map(|(entry_id, _)| entry_id + 1).max().unwrap_or(0);
        entries = vec![None; length];
        for (entry_id, offset) in pairs {
            entries[entry_id] = Some(parse_entry(data, entries_base + offset, chunk_end)?);
        }
    } else if flags & TYPE_FLAG_OFFSET16 != 0 {
        for (index, slot) in entries.iter_mut().enumerate() {
            let raw = u16_at(data, offsets_start + index * 2)?;
            if raw != u16::MAX {
                *slot = Some(parse_entry(
                    data,
                    entries_base + usize::from(raw) * 4,
                    chunk_end,
                )?);
            }
        }
    } else {
        for (index, slot) in entries.iter_mut().enumerate() {
            let raw = u32_at(data, offsets_start + index * 4)?;
            if raw != NO_ENTRY {
                *slot = Some(parse_entry(data, entries_base + raw as usize, chunk_end)?);
            }
        }
    }

    Ok(TypeBlock {
        id,
        reserved,
        config,
        entries,
    })
}

fn parse_entry(data: &[u8], position: usize, limit: usize) -> Result<Entry> {
    let raw_flags = u16_at(data, position + 2)?;
    if raw_flags & FLAG_COMPACT != 0 {
        if position + 8 > limit {
            return Err(malformed("compact resource entry overruns its type block"));
        }
        let key = u32::from(u16_at(data, position)?);
        let data_type = u8_at(data, position + 3)?;
        let value = ResValue::new(data_type, u32_at(data, position + 4)?);
        return Ok(Entry {
            flags: raw_flags & 0x00FF & !FLAG_COMPACT,
            key,
            value: EntryValue::Simple(value),
            extra: Vec::new(),
        });
    }
    let size = usize::from(u16_at(data, position)?);
    let flags = raw_flags;
    let key = u32_at(data, position + 4)?;
    if position + size > limit {
        return Err(malformed("resource entry overruns its type block"));
    }
    if flags & FLAG_COMPLEX != 0 {
        let parent = u32_at(data, position + 8)?;
        let count = u32_at(data, position + 12)? as usize;
        if size < 16 {
            return Err(malformed("map entry header is smaller than 16 bytes"));
        }
        if position + size + count * 12 > limit {
            return Err(malformed("map entry items overrun their type block"));
        }
        let mut items = Vec::with_capacity(count.min(1024));
        for index in 0..count {
            let base = position + size + index * 12;
            items.push(MapItem {
                name: u32_at(data, base)?,
                value: read_value(data, base + 4)?,
            });
        }
        let extra = Vec::new();
        Ok(Entry {
            flags,
            key,
            value: EntryValue::Map { parent, items },
            extra,
        })
    } else {
        if size < 8 {
            return Err(malformed("simple entry header is smaller than 8 bytes"));
        }
        let value_at = position + size;
        if value_at + 8 > limit {
            return Err(malformed("simple entry value overruns its type block"));
        }
        let value = read_value(data, value_at)?;
        Ok(Entry {
            flags,
            key,
            value: EntryValue::Simple(value),
            extra: Vec::new(),
        })
    }
}

fn read_value(data: &[u8], position: usize) -> Result<ResValue> {
    Ok(ResValue {
        size: u16_at(data, position)?,
        res0: u8_at(data, position + 2)?,
        data_type: u8_at(data, position + 3)?,
        data: u32_at(data, position + 4)?,
    })
}

fn read_pool(data: &[u8], position: usize) -> Result<StringPool> {
    if u16_at(data, position)? != RES_STRING_POOL {
        return Err(malformed("expected a string pool chunk"));
    }
    let header_size = usize::from(u16_at(data, position + 2)?);
    let size = chunk_size(data, position)?;
    if header_size < POOL_HEADER || header_size > size {
        return Err(malformed("invalid string pool header size"));
    }
    let chunk = &data[position..position + size];
    let string_count = u32_at(chunk, 8)? as usize;
    let style_count = u32_at(chunk, 12)? as usize;
    let flags = u32_at(chunk, 16)?;
    let strings_start = u32_at(chunk, 20)? as usize;
    let styles_start = u32_at(chunk, 24)? as usize;
    let utf8 = flags & UTF8_FLAG != 0;
    let sorted = flags & SORTED_FLAG != 0;

    let mut strings = Vec::new();
    for index in 0..string_count {
        let relative = u32_at(chunk, header_size + index * 4)? as usize;
        strings.push(read_string(chunk, strings_start + relative, utf8)?);
    }

    let style_offsets = header_size + string_count * 4;
    let mut styles = Vec::new();
    for index in 0..style_count {
        let relative = u32_at(chunk, style_offsets + index * 4)? as usize;
        let mut cursor = styles_start + relative;
        let mut words = Vec::new();
        loop {
            let word = u32_at(chunk, cursor)?;
            words.push(word);
            cursor += 4;
            if word == NO_INDEX {
                break;
            }
        }
        styles.push(words);
    }

    Ok(StringPool {
        strings,
        styles,
        utf8,
        sorted,
    })
}

fn read_string(chunk: &[u8], position: usize, utf8: bool) -> Result<String> {
    let mut cursor = position;
    if utf8 {
        let _utf16_length = read_length8(chunk, &mut cursor)?;
        let byte_length = read_length8(chunk, &mut cursor)?;
        let bytes = slice(chunk, cursor, byte_length)?;
        Ok(crate::util::decode_utf8_lenient(bytes))
    } else {
        let length = read_length16(chunk, &mut cursor)?;
        let mut units = Vec::with_capacity(length);
        for index in 0..length {
            units.push(u16_at(chunk, cursor + index * 2)?);
        }
        String::from_utf16(&units)
            .map_err(|_| malformed("invalid UTF-16 string in a resource pool"))
    }
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

fn read_length16(chunk: &[u8], cursor: &mut usize) -> Result<usize> {
    let first = usize::from(u16_at(chunk, *cursor)?);
    *cursor += 2;
    if first & 0x8000 != 0 {
        let second = usize::from(u16_at(chunk, *cursor)?);
        *cursor += 2;
        Ok(((first & 0x7FFF) << 16) | second)
    } else {
        Ok(first)
    }
}

fn encode_pool(pool: &StringPool) -> Vec<u8> {
    let mut offsets = Vec::new();
    let mut data = Vec::new();
    for text in &pool.strings {
        put_u32(&mut offsets, data.len() as u32);
        if pool.utf8 {
            write_utf8(&mut data, text);
        } else {
            write_utf16(&mut data, text);
        }
    }
    while data.len() % 4 != 0 {
        data.push(0);
    }

    let mut style_offsets = Vec::new();
    let mut style_data = Vec::new();
    for style in &pool.styles {
        put_u32(&mut style_offsets, style_data.len() as u32);
        for word in style {
            put_u32(&mut style_data, *word);
        }
    }

    let strings_start = (POOL_HEADER + offsets.len() + style_offsets.len()) as u32;
    let styles_start = if pool.styles.is_empty() {
        0
    } else {
        strings_start + data.len() as u32
    };
    let mut flags = 0u32;
    if pool.utf8 {
        flags |= UTF8_FLAG;
    }
    if pool.sorted {
        flags |= SORTED_FLAG;
    }

    let mut rest = Vec::new();
    put_u32(&mut rest, pool.strings.len() as u32);
    put_u32(&mut rest, pool.styles.len() as u32);
    put_u32(&mut rest, flags);
    put_u32(&mut rest, strings_start);
    put_u32(&mut rest, styles_start);
    rest.extend(offsets);
    rest.extend(style_offsets);
    rest.extend(data);
    rest.extend(style_data);
    chunk(RES_STRING_POOL, POOL_HEADER as u16, &rest)
}

fn write_utf8(out: &mut Vec<u8>, text: &str) {
    let utf16_length = text.encode_utf16().count();
    write_length8(out, utf16_length);
    write_length8(out, text.len());
    out.extend_from_slice(text.as_bytes());
    out.push(0);
}

fn write_length8(out: &mut Vec<u8>, length: usize) {
    if length < 0x80 {
        out.push(length as u8);
    } else {
        out.push(0x80 | ((length >> 8) as u8));
        out.push((length & 0xFF) as u8);
    }
}

fn write_utf16(out: &mut Vec<u8>, text: &str) {
    let units: Vec<u16> = text.encode_utf16().collect();
    let length = units.len();
    if length < 0x8000 {
        put_u16(out, length as u16);
    } else {
        put_u16(out, 0x8000 | ((length >> 16) as u16));
        put_u16(out, (length & 0xFFFF) as u16);
    }
    for unit in units {
        put_u16(out, unit);
    }
    put_u16(out, 0);
}

fn encode_package(package: &Package) -> Vec<u8> {
    let type_pool = encode_pool(&package.type_strings);
    let key_pool = encode_pool(&package.key_strings);
    let header_length = PACKAGE_HEADER + package.header_extra.len();
    let type_offset = header_length;
    let key_offset = type_offset + type_pool.len();

    let mut rest = Vec::new();
    put_u32(&mut rest, package.id);
    rest.extend(name_bytes(&package.name));
    put_u32(&mut rest, type_offset as u32);
    put_u32(&mut rest, package.last_public_type);
    put_u32(&mut rest, key_offset as u32);
    put_u32(&mut rest, package.last_public_key);
    put_u32(&mut rest, package.type_id_offset);
    rest.extend(&package.header_extra);
    rest.extend(type_pool);
    rest.extend(key_pool);
    for child in &package.children {
        match child {
            Child::TypeSpec(spec) => rest.extend(encode_type_spec(spec)),
            Child::Type(block) => rest.extend(encode_type(block)),
            Child::Raw(bytes) => rest.extend_from_slice(bytes),
        }
    }
    chunk(RES_TABLE_PACKAGE, header_length as u16, &rest)
}

fn encode_type_spec(spec: &TypeSpec) -> Vec<u8> {
    let mut rest = vec![spec.id, spec.res0];
    put_u16(&mut rest, spec.res1);
    put_u32(&mut rest, spec.flags.len() as u32);
    for flag in &spec.flags {
        put_u32(&mut rest, *flag);
    }
    chunk(RES_TABLE_TYPE_SPEC, TYPE_SPEC_HEADER as u16, &rest)
}

fn encode_type(block: &TypeBlock) -> Vec<u8> {
    let config = &block.config.raw;
    let header_length = TYPE_HEADER + config.len();
    let count = block.entries.len();
    let entries_start = header_length + count * 4;

    let mut offsets = Vec::new();
    let mut data = Vec::new();
    for entry in &block.entries {
        match entry {
            Some(entry) => {
                put_u32(&mut offsets, data.len() as u32);
                data.extend(encode_entry(entry));
            }
            None => put_u32(&mut offsets, NO_ENTRY),
        }
    }

    let mut rest = vec![block.id, 0];
    put_u16(&mut rest, block.reserved);
    put_u32(&mut rest, count as u32);
    put_u32(&mut rest, entries_start as u32);
    rest.extend_from_slice(config);
    rest.extend(offsets);
    rest.extend(data);
    chunk(RES_TABLE_TYPE, header_length as u16, &rest)
}

fn encode_entry(entry: &Entry) -> Vec<u8> {
    match &entry.value {
        EntryValue::Simple(value) => {
            let mut out = Vec::with_capacity(16);
            put_u16(&mut out, 8);
            put_u16(&mut out, entry.flags & !FLAG_COMPLEX);
            put_u32(&mut out, entry.key);
            put_value(&mut out, value);
            out
        }
        EntryValue::Map { parent, items } => {
            let size = 16usize;
            let mut out = Vec::with_capacity(size + items.len() * 12 + entry.extra.len());
            put_u16(&mut out, size as u16);
            put_u16(&mut out, entry.flags | FLAG_COMPLEX);
            put_u32(&mut out, entry.key);
            put_u32(&mut out, *parent);
            put_u32(&mut out, items.len() as u32);
            for item in items {
                put_u32(&mut out, item.name);
                put_value(&mut out, &item.value);
            }
            out.extend_from_slice(&entry.extra);
            out
        }
    }
}

fn name_bytes(name: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(NAME_UNITS * 2);
    for unit in name.encode_utf16().take(NAME_UNITS - 1) {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out.resize(NAME_UNITS * 2, 0);
    out
}

fn decode_name(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
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

fn put_value(out: &mut Vec<u8>, value: &ResValue) {
    put_u16(out, value.size);
    out.push(value.res0);
    out.push(value.data_type);
    put_u32(out, value.data);
}

fn header_u32(data: &[u8], start: usize, header_size: usize, offset: usize) -> Result<u32> {
    if header_size >= offset + 4 {
        u32_at(data, start + offset)
    } else {
        Ok(0)
    }
}

fn chunk_size(data: &[u8], position: usize) -> Result<usize> {
    let size = u32_at(data, position + 4)? as usize;
    if size < 8 {
        return Err(malformed("chunk is smaller than its header"));
    }
    slice(data, position, size)?;
    Ok(size)
}

fn slice(data: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    offset
        .checked_add(length)
        .and_then(|end| data.get(offset..end))
        .ok_or_else(|| truncated(offset))
}

fn u8_at(data: &[u8], offset: usize) -> Result<u8> {
    data.get(offset).copied().ok_or_else(|| truncated(offset))
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16> {
    let bytes = slice(data, offset, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = slice(data, offset, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn malformed(message: impl Into<String>) -> Error {
    Error::Malformed(message.into())
}

fn truncated(offset: usize) -> Error {
    malformed(format!("data ends unexpectedly at offset {offset:#x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_table() -> Table {
        let global = StringPool::from_strings(
            vec![
                "Hello".to_owned(),
                "héllo wörld".to_owned(),
                "x".repeat(200),
                "😀 emoji".to_owned(),
            ],
            true,
        );
        let type_strings = StringPool::from_strings(vec!["string".to_owned(), "layout".to_owned()], true);
        let key_strings = StringPool::from_strings(
            vec!["app_name".to_owned(), "main".to_owned(), "unused".to_owned()],
            true,
        );
        let mut german = ResConfig::zeroed();
        german.raw[8..10].copy_from_slice(b"de");
        german.raw[14..16].copy_from_slice(&240u16.to_le_bytes());
        let string_entry = Entry {
            flags: 0,
            key: 0,
            value: EntryValue::Simple(ResValue::new(DATA_TYPE_STRING, 0)),
            extra: Vec::new(),
        };
        let map_entry = Entry {
            flags: FLAG_COMPLEX,
            key: 1,
            value: EntryValue::Map {
                parent: 0x0100_0000,
                items: vec![MapItem {
                    name: 0x0101_0000,
                    value: ResValue::new(0x10, 5),
                }],
            },
            extra: Vec::new(),
        };
        let package = Package {
            id: 0x7f,
            name: "com.example.app".to_owned(),
            last_public_type: 2,
            last_public_key: 2,
            type_id_offset: 0,
            header_extra: Vec::new(),
            type_strings,
            key_strings,
            children: vec![
                Child::TypeSpec(TypeSpec {
                    id: 1,
                    res0: 0,
                    res1: 0,
                    flags: vec![0, 0x4000_0000],
                }),
                Child::Type(TypeBlock {
                    id: 1,
                    reserved: 0,
                    config: ResConfig::zeroed(),
                    entries: vec![Some(string_entry), None],
                }),
                Child::Type(TypeBlock {
                    id: 2,
                    reserved: 0,
                    config: german,
                    entries: vec![Some(map_entry)],
                }),
                Child::Raw(vec![0x03, 0x02, 0x0c, 0x00, 0x0c, 0x00, 0x00, 0x00, 0, 0, 0, 0]),
            ],
        };
        Table {
            global,
            packages: vec![package],
        }
    }

    #[test]
    fn utf8_pool_round_trips() {
        let pool = StringPool::from_strings(
            vec![
                "Hello".to_owned(),
                "héllo wörld".to_owned(),
                "x".repeat(200),
                String::new(),
                "😀 emoji".to_owned(),
            ],
            true,
        );
        let bytes = encode_pool(&pool);
        assert_eq!(read_pool(&bytes, 0).expect("pool"), pool);
    }

    #[test]
    fn utf16_pool_round_trips_long_lengths() {
        let pool = StringPool::from_strings(
            vec!["ünï".to_owned(), "😀".to_owned(), "a".repeat(33_000)],
            false,
        );
        let bytes = encode_pool(&pool);
        assert_eq!(read_pool(&bytes, 0).expect("pool"), pool);
    }

    #[test]
    fn styles_survive_round_trip() {
        let mut pool = StringPool::from_strings(vec!["one".to_owned(), "two".to_owned()], true);
        pool.styles = vec![vec![1, 2, 3, NO_INDEX], vec![NO_INDEX]];
        let bytes = encode_pool(&pool);
        assert_eq!(read_pool(&bytes, 0).expect("pool"), pool);
    }

    #[test]
    fn table_round_trips_structurally() {
        let table = sample_table();
        let bytes = serialize(&table);
        assert_eq!(parse(&bytes).expect("parse"), table);
    }

    #[test]
    fn serialization_is_canonical_after_one_pass() {
        let first = serialize(&sample_table());
        let parsed = parse(&first).expect("parse");
        let second = serialize(&parsed);
        assert_eq!(first, second);
    }

    #[test]
    fn truncated_table_is_rejected() {
        let bytes = serialize(&sample_table());
        assert!(parse(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn wrong_magic_is_rejected() {
        let mut bytes = serialize(&sample_table());
        bytes[0] = 0x00;
        assert!(parse(&bytes).is_err());
    }

    #[test]
    #[ignore]
    fn round_trips_a_real_resource_table() {
        let path = std::env::var("ANTISPLIT_RS_ARSC")
            .expect("set ANTISPLIT_RS_ARSC to the path of a resources.arsc file");
        let bytes = std::fs::read(path).expect("read resources.arsc");
        let table = parse(&bytes).expect("parse");
        let once = serialize(&table);
        let again = parse(&once).expect("reparse");
        assert_eq!(again, table);
        assert_eq!(serialize(&again), once);
    }
}

#[cfg(test)]
mod map_entry_size_tests {
    use super::*;

    #[test]
    fn map_entry_header_size_is_16_and_items_follow_it() {
        let entry = Entry {
            flags: 0,
            key: 7,
            value: EntryValue::Map {
                parent: 0,
                items: vec![MapItem {
                    name: 0x0100_0000,
                    value: ResValue::new(0x01, 0x7f09_0009),
                }],
            },
            extra: Vec::new(),
        };
        let bytes = encode_entry(&entry);
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), 16);
        assert_eq!(bytes.len(), 16 + 12);
        let parsed = parse_entry(&bytes, 0, bytes.len()).expect("parse");
        assert_eq!(parsed.value, entry.value);
    }
}
