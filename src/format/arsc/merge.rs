use std::collections::{BTreeMap, HashMap};

use crate::format::arsc::format::{
    Child, DATA_TYPE_STRING, Entry, EntryValue, MapItem, NO_INDEX, Package, ResConfig, ResValue,
    StringPool, Table, TypeBlock, TypeSpec,
};
use crate::error::{Error, Result};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MergeStats {
    pub tables: usize,
    pub packages: usize,
    pub entries_added: usize,
    pub blocks_added: usize,
    pub collisions: usize,
}

pub fn merge(base: &Table, others: &[Table]) -> Result<(Table, MergeStats)> {
    let mut stats = MergeStats {
        tables: others.len() + 1,
        ..MergeStats::default()
    };
    let mut global = PoolBuilder::new(base.global.utf8);
    let mut packages: Vec<PackageAcc> = Vec::new();
    absorb_table(&mut global, &mut packages, base, true, &mut stats)?;
    for table in others {
        absorb_table(&mut global, &mut packages, table, false, &mut stats)?;
    }
    stats.packages = packages.len();
    let merged = Table {
        global: global.finish(),
        packages: packages.into_iter().map(PackageAcc::finish).collect(),
    };
    Ok((merged, stats))
}

pub fn verify_type_names(base: &Table, merged: &Table) -> Result<()> {
    for package in &base.packages {
        let Some(output) = merged
            .packages
            .iter()
            .find(|candidate| candidate.id == package.id)
        else {
            return Err(Error::Verification(format!(
                "package {:#04x} is missing from the merged table",
                package.id
            )));
        };
        for child in &package.children {
            let id = match child {
                Child::TypeSpec(spec) => spec.id,
                Child::Type(block) => block.id,
                Child::Raw(_) => continue,
            };
            let before = type_name_of(package, id);
            if before.is_some_and(is_placeholder_type_name) {
                continue;
            }
            let after = type_name_of(output, id);
            if before != after {
                return Err(Error::Verification(format!(
                    "type id {id} is named {after:?} in the merged table but {before:?} in the base"
                )));
            }
        }
    }
    Ok(())
}

fn type_name_of(package: &Package, id: u8) -> Option<&str> {
    let slot = usize::from(id).checked_sub(1 + package.type_id_offset as usize)?;
    package.type_strings.strings.get(slot).map(String::as_str)
}

fn absorb_table(
    global: &mut PoolBuilder,
    packages: &mut Vec<PackageAcc>,
    table: &Table,
    is_base: bool,
    stats: &mut MergeStats,
) -> Result<()> {
    let strings = import(global, &table.global, !is_base);
    for package in &table.packages {
        let position = match packages.iter().position(|acc| acc.id == package.id) {
            Some(position) => position,
            None => {
                packages.push(PackageAcc::new(package));
                packages.len() - 1
            }
        };
        let acc = &mut packages[position];
        if acc.name != package.name {
            return Err(Error::Incompatible(format!(
                "package {:#04x} is named {} and {}",
                package.id, acc.name, package.name
            )));
        }
        if acc.type_id_offset == package.type_id_offset {
            acc.absorb_names(package)?;
        }
        let keys = import(&mut acc.keys, &package.key_strings, !is_base);
        acc.absorb_children(package, &strings, &keys, is_base, stats)?;
    }
    Ok(())
}

struct PoolBuilder {
    utf8: bool,
    strings: Vec<String>,
    styles: Vec<Vec<u32>>,
    index: HashMap<(String, Vec<u32>), u32>,
}

impl PoolBuilder {
    fn new(utf8: bool) -> Self {
        Self {
            utf8,
            strings: Vec::new(),
            styles: Vec::new(),
            index: HashMap::new(),
        }
    }

    fn append(&mut self, text: &str, style: Vec<u32>) -> u32 {
        let position = self.strings.len() as u32;
        self.index
            .entry((text.to_owned(), style.clone()))
            .or_insert(position);
        self.strings.push(text.to_owned());
        self.styles.push(style);
        position
    }

    fn intern(&mut self, text: &str, style: Vec<u32>) -> u32 {
        if let Some(position) = self.index.get(&(text.to_owned(), style.clone())) {
            return *position;
        }
        self.append(text, style)
    }

    fn finish(self) -> StringPool {
        let any_style = self.styles.iter().any(|style| !style.is_empty());
        let styles = if any_style {
            self.styles
                .into_iter()
                .map(|style| if style.is_empty() { vec![NO_INDEX] } else { style })
                .collect()
        } else {
            Vec::new()
        };
        StringPool {
            strings: self.strings,
            styles,
            utf8: self.utf8,
            sorted: false,
        }
    }
}

fn import(builder: &mut PoolBuilder, pool: &StringPool, dedupe: bool) -> Vec<u32> {
    (0..pool.strings.len())
        .map(|index| {
            let text = &pool.strings[index];
            let style = pool.style_of(index);
            if dedupe {
                builder.intern(text, style)
            } else {
                builder.append(text, style)
            }
        })
        .collect()
}

#[derive(Default)]
struct TypeAcc {
    flags: Vec<u32>,
    blocks: Vec<BlockAcc>,
}

struct BlockAcc {
    key: Vec<u8>,
    reserved: u16,
    config: ResConfig,
    entries: Vec<Option<Entry>>,
}

struct PackageAcc {
    id: u32,
    name: String,
    last_public_type: u32,
    last_public_key: u32,
    header_extra: Vec<u8>,
    type_id_offset: u32,
    type_utf8: bool,
    type_names: Vec<String>,
    keys: PoolBuilder,
    types: BTreeMap<u8, TypeAcc>,
    raw: Vec<Vec<u8>>,
}

impl PackageAcc {
    fn new(package: &Package) -> Self {
        Self {
            id: package.id,
            name: package.name.clone(),
            last_public_type: package.last_public_type,
            last_public_key: package.last_public_key,
            header_extra: package.header_extra.clone(),
            type_id_offset: package.type_id_offset,
            type_utf8: package.type_strings.utf8,
            type_names: Vec::new(),
            keys: PoolBuilder::new(package.key_strings.utf8),
            types: BTreeMap::new(),
            raw: Vec::new(),
        }
    }

    fn absorb_names(&mut self, package: &Package) -> Result<()> {
        for (index, name) in package.type_strings.strings.iter().enumerate() {
            let placeholder = is_placeholder_type_name(name);
            match self.type_names.get(index) {
                Some(existing) if existing == name => {}
                Some(existing) if is_placeholder_type_name(existing) => {
                    if !placeholder {
                        self.type_names[index] = name.clone();
                    }
                }
                Some(_) if placeholder => {}
                Some(existing) => {
                    return Err(Error::Incompatible(format!(
                        "type {} is named {existing} in one table and {name} in another",
                        index + 1
                    )));
                }
                None => self.type_names.push(name.clone()),
            }
        }
        Ok(())
    }

    fn type_id_for(&mut self, source: &Package, source_id: u8) -> Result<u8> {
        if source.type_id_offset == self.type_id_offset {
            return Ok(source_id);
        }
        let source_slot = usize::from(source_id)
            .checked_sub(1 + source.type_id_offset as usize)
            .ok_or_else(|| {
                Error::Malformed(format!(
                    "type id {source_id} is below the package type offset {}",
                    source.type_id_offset
                ))
            })?;
        let slot = match source.type_strings.strings.get(source_slot) {
            Some(name) if !is_placeholder_type_name(name) => self.slot_for_name(name),
            _ => {
                self.ensure_slot(source_slot);
                source_slot
            }
        };
        let id = slot + 1 + self.type_id_offset as usize;
        match u8::try_from(id) {
            Ok(id) if id != 0 => Ok(id),
            _ => Err(Error::Incompatible(format!(
                "type id {id} does not fit in one byte"
            ))),
        }
    }

    fn slot_for_name(&mut self, name: &str) -> usize {
        if let Some(index) = self.type_names.iter().position(|existing| existing == name) {
            return index;
        }
        self.type_names.push(name.to_owned());
        self.type_names.len() - 1
    }

    fn ensure_slot(&mut self, slot: usize) {
        while self.type_names.len() <= slot {
            let index = self.type_names.len();
            self.type_names.push(format!("?{}", index + 1));
        }
    }

    fn absorb_children(
        &mut self,
        package: &Package,
        strings: &[u32],
        keys: &[u32],
        is_base: bool,
        stats: &mut MergeStats,
    ) -> Result<()> {
        for child in &package.children {
            match child {
                Child::TypeSpec(spec) => {
                    let id = self.type_id_for(package, spec.id)?;
                    let target = self.types.entry(id).or_default();
                    if target.flags.len() < spec.flags.len() {
                        target.flags.resize(spec.flags.len(), 0);
                    }
                    for (slot, flag) in target.flags.iter_mut().zip(spec.flags.iter()) {
                        *slot |= *flag;
                    }
                }
                Child::Type(block) => {
                    let id = self.type_id_for(package, block.id)?;
                    let mut incoming = Vec::with_capacity(block.entries.len());
                    for entry in &block.entries {
                        incoming.push(match entry {
                            Some(entry) => Some(remap_entry(entry, strings, keys)?),
                            None => None,
                        });
                    }
                    absorb_block(self.types.entry(id).or_default(), block, incoming, stats);
                }
                Child::Raw(bytes) => {
                    if is_base {
                        self.raw.push(bytes.clone());
                    }
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Package {
        let mut children = Vec::new();
        for (id, target) in self.types {
            let count = target
                .blocks
                .iter()
                .map(|block| block.entries.len())
                .chain(std::iter::once(target.flags.len()))
                .max()
                .unwrap_or(0);
            let mut flags = target.flags;
            flags.resize(count, 0);
            children.push(Child::TypeSpec(TypeSpec {
                id,
                res0: 0,
                res1: 0,
                flags,
            }));
            for block in target.blocks {
                let mut entries = block.entries;
                entries.resize(count, None);
                children.push(Child::Type(TypeBlock {
                    id,
                    reserved: block.reserved,
                    config: block.config,
                    entries,
                }));
            }
        }
        children.extend(self.raw.into_iter().map(Child::Raw));
        Package {
            id: self.id,
            name: self.name,
            last_public_type: self.last_public_type,
            last_public_key: self.last_public_key,
            type_id_offset: self.type_id_offset,
            header_extra: self.header_extra,
            type_strings: StringPool::from_strings(self.type_names, self.type_utf8),
            key_strings: self.keys.finish(),
            children,
        }
    }
}

fn absorb_block(
    target: &mut TypeAcc,
    block: &TypeBlock,
    incoming: Vec<Option<Entry>>,
    stats: &mut MergeStats,
) {
    let key = config_key(&block.config);
    let index = match target.blocks.iter().position(|existing| existing.key == key) {
        Some(index) => index,
        None => {
            target.blocks.push(BlockAcc {
                key,
                reserved: block.reserved,
                config: block.config.clone(),
                entries: Vec::new(),
            });
            stats.blocks_added += 1;
            target.blocks.len() - 1
        }
    };
    let existing = &mut target.blocks[index];
    if existing.entries.len() < incoming.len() {
        existing.entries.resize(incoming.len(), None);
    }
    for (slot, candidate) in existing.entries.iter_mut().zip(incoming) {
        match (slot.is_some(), candidate) {
            (false, Some(entry)) => {
                *slot = Some(entry);
                stats.entries_added += 1;
            }
            (true, Some(_)) => stats.collisions += 1,
            _ => {}
        }
    }
}

fn remap_entry(entry: &Entry, strings: &[u32], keys: &[u32]) -> Result<Entry> {
    let key = *keys
        .get(entry.key as usize)
        .ok_or_else(|| Error::Malformed(format!("entry key {} is out of range", entry.key)))?;
    let value = match &entry.value {
        EntryValue::Simple(value) => EntryValue::Simple(remap_value(value, strings)?),
        EntryValue::Map { parent, items } => EntryValue::Map {
            parent: *parent,
            items: items
                .iter()
                .map(|item| -> Result<MapItem> {
                    Ok(MapItem {
                        name: item.name,
                        value: remap_value(&item.value, strings)?,
                    })
                })
                .collect::<Result<Vec<MapItem>>>()?,
        },
    };
    Ok(Entry {
        flags: entry.flags,
        key,
        value,
        extra: entry.extra.clone(),
    })
}

fn remap_value(value: &ResValue, strings: &[u32]) -> Result<ResValue> {
    if value.data_type != DATA_TYPE_STRING {
        return Ok(value.clone());
    }
    let data = *strings
        .get(value.data as usize)
        .ok_or_else(|| Error::Malformed(format!("string index {} is out of range", value.data)))?;
    Ok(ResValue {
        size: value.size,
        res0: value.res0,
        data_type: value.data_type,
        data,
    })
}

fn config_key(config: &ResConfig) -> Vec<u8> {
    let mut body = config.raw.get(4..).unwrap_or(&[]).to_vec();
    while body.last() == Some(&0) {
        body.pop();
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::arsc::format::{DATA_TYPE_STRING, Entry, EntryValue, ResValue};

    fn string_entry(key: u32, index: u32) -> Entry {
        Entry {
            flags: 0,
            key,
            value: EntryValue::Simple(ResValue::new(DATA_TYPE_STRING, index)),
            extra: Vec::new(),
        }
    }

    fn config_with_language(language: &[u8; 2]) -> ResConfig {
        let mut config = ResConfig::zeroed();
        config.raw[8..10].copy_from_slice(language);
        config
    }

    fn package(name: &str, keys: &[&str], children: Vec<Child>) -> Package {
        Package {
            id: 0x7f,
            name: name.to_owned(),
            last_public_type: 1,
            last_public_key: 1,
            type_id_offset: 0,
            header_extra: Vec::new(),
            type_strings: StringPool::from_strings(vec!["string".to_owned()], true),
            key_strings: StringPool::from_strings(
                keys.iter().map(|key| (*key).to_owned()).collect(),
                true,
            ),
            children,
        }
    }

    fn spec(count: usize) -> Child {
        Child::TypeSpec(TypeSpec {
            id: 1,
            res0: 0,
            res1: 0,
            flags: vec![0; count],
        })
    }

    fn block(config: ResConfig, entries: Vec<Option<Entry>>) -> Child {
        Child::Type(TypeBlock {
            id: 1,
            reserved: 0,
            config,
            entries,
        })
    }

    fn table(strings: &[&str], package: Package) -> Table {
        Table {
            global: StringPool::from_strings(
                strings.iter().map(|text| (*text).to_owned()).collect(),
                true,
            ),
            packages: vec![package],
        }
    }

    fn text_of(table: &Table, config: &ResConfig, index: usize) -> Option<String> {
        table.packages[0].children.iter().find_map(|child| match child {
            Child::Type(block) if config_key(&block.config) == config_key(config) => block
                .entries
                .get(index)?
                .as_ref()
                .and_then(|entry| match &entry.value {
                    EntryValue::Simple(value) => table.global.strings.get(value.data as usize).cloned(),
                    _ => None,
                }),
            _ => None,
        })
    }

    fn key_of(table: &Table, config: &ResConfig, index: usize) -> Option<u32> {
        table.packages[0].children.iter().find_map(|child| match child {
            Child::Type(block) if config_key(&block.config) == config_key(config) => block
                .entries
                .get(index)?
                .as_ref()
                .map(|entry| entry.key),
            _ => None,
        })
    }

    #[test]
    fn split_configs_are_added_with_remapped_strings() {
        let base = table(
            &["Hello"],
            package(
                "com.example",
                &["app_name"],
                vec![spec(1), block(ResConfig::zeroed(), vec![Some(string_entry(0, 0))])],
            ),
        );
        let french = table(
            &["Bonjour"],
            package(
                "com.example",
                &["app_name"],
                vec![
                    spec(1),
                    block(config_with_language(b"fr"), vec![Some(string_entry(0, 0))]),
                ],
            ),
        );
        let (merged, stats) = merge(&base, &[french]).expect("merge");
        assert_eq!(text_of(&merged, &ResConfig::zeroed(), 0).as_deref(), Some("Hello"));
        assert_eq!(
            text_of(&merged, &config_with_language(b"fr"), 0).as_deref(),
            Some("Bonjour")
        );
        assert_eq!(stats.blocks_added, 2);
        assert_eq!(stats.collisions, 0);
    }

    #[test]
    fn collisions_keep_the_base_value() {
        let base = table(
            &["A"],
            package(
                "com.example",
                &["app_name"],
                vec![spec(1), block(ResConfig::zeroed(), vec![Some(string_entry(0, 0))])],
            ),
        );
        let split = table(
            &["B"],
            package(
                "com.example",
                &["app_name"],
                vec![spec(1), block(ResConfig::zeroed(), vec![Some(string_entry(0, 0))])],
            ),
        );
        let (merged, stats) = merge(&base, &[split]).expect("merge");
        assert_eq!(text_of(&merged, &ResConfig::zeroed(), 0).as_deref(), Some("A"));
        assert_eq!(stats.collisions, 1);
    }

    #[test]
    fn keys_are_deduplicated_and_remapped() {
        let base = table(
            &["A"],
            package(
                "com.example",
                &["app_name"],
                vec![
                    spec(2),
                    block(
                        ResConfig::zeroed(),
                        vec![Some(string_entry(0, 0)), None],
                    ),
                ],
            ),
        );
        let split = table(
            &["B"],
            package(
                "com.example",
                &["other", "app_name"],
                vec![
                    spec(2),
                    block(
                        ResConfig::zeroed(),
                        vec![None, Some(string_entry(1, 0))],
                    ),
                ],
            ),
        );
        let (merged, _) = merge(&base, &[split]).expect("merge");
        assert_eq!(key_of(&merged, &ResConfig::zeroed(), 1), Some(0));
        assert_eq!(merged.packages[0].key_strings.strings.len(), 2);
        assert_eq!(text_of(&merged, &ResConfig::zeroed(), 1).as_deref(), Some("B"));
    }

    #[test]
    fn package_name_mismatch_is_rejected() {
        let base = table(&["A"], package("com.example", &["k"], vec![spec(1)]));
        let other = table(&["B"], package("com.other", &["k"], vec![spec(1)]));
        assert!(merge(&base, &[other]).is_err());
    }

    fn typed_package(
        offset: u32,
        type_names: &[&str],
        keys: &[&str],
        children: Vec<Child>,
    ) -> Package {
        let mut package = package("com.example", keys, children);
        package.type_id_offset = offset;
        package.type_strings = StringPool::from_strings(
            type_names.iter().map(|name| (*name).to_owned()).collect(),
            true,
        );
        package
    }

    fn string_type(id: u8, config: ResConfig, index: u32) -> Child {
        Child::Type(TypeBlock {
            id,
            reserved: 0,
            config,
            entries: vec![Some(string_entry(0, index))],
        })
    }

    fn string_spec(id: u8) -> Child {
        Child::TypeSpec(TypeSpec {
            id,
            res0: 0,
            res1: 0,
            flags: vec![0],
        })
    }

    fn entry_text(table: &Table, type_id: u8, config: &ResConfig) -> Option<String> {
        table.packages[0].children.iter().find_map(|child| match child {
            Child::Type(block)
                if block.id == type_id && config_key(&block.config) == config_key(config) =>
            {
                block
                    .entries
                    .first()?
                    .as_ref()
                    .and_then(|entry| match &entry.value {
                        EntryValue::Simple(value) => {
                            table.global.strings.get(value.data as usize).cloned()
                        }
                        _ => None,
                    })
            }
            _ => None,
        })
    }

    #[test]
    fn base_type_id_offset_is_kept() {
        let base = table(
            &["Hello"],
            typed_package(
                1,
                &["string"],
                &["app_name"],
                vec![string_spec(2), string_type(2, ResConfig::zeroed(), 0)],
            ),
        );
        let (merged, _) = merge(&base, &[]).expect("merge");
        assert_eq!(merged.packages[0].type_id_offset, 1);
        assert!(verify_type_names(&base, &merged).is_ok());
    }

    #[test]
    fn split_with_another_type_offset_is_merged_by_name() {
        let base = table(
            &["Hello"],
            typed_package(
                0,
                &["attr", "string"],
                &["app_name"],
                vec![string_spec(2), string_type(2, ResConfig::zeroed(), 0)],
            ),
        );
        let french = table(
            &["Bonjour"],
            typed_package(
                1,
                &["dimen", "string"],
                &["app_name"],
                vec![string_spec(3), string_type(3, config_with_language(b"fr"), 0)],
            ),
        );
        let (merged, _) = merge(&base, &[french]).expect("merge");
        let package = &merged.packages[0];
        assert_eq!(package.type_id_offset, 0);
        assert_eq!(
            package.type_strings.strings,
            vec!["attr".to_owned(), "string".to_owned()]
        );
        assert_eq!(
            entry_text(&merged, 2, &config_with_language(b"fr")).as_deref(),
            Some("Bonjour")
        );
        assert_eq!(
            entry_text(&merged, 2, &ResConfig::zeroed()).as_deref(),
            Some("Hello")
        );
        assert!(verify_type_names(&base, &merged).is_ok());
    }

    #[test]
    #[ignore]
    fn real_table_keeps_its_type_ids() {
        let path = std::env::var("ANTISPLIT_RS_ARSC")
            .expect("set ANTISPLIT_RS_ARSC to the path of a base resources.arsc");
        let bytes = std::fs::read(path).expect("read resources.arsc");
        let base = crate::format::arsc::format::parse(&bytes).expect("parse");
        let (merged, _) = merge(&base, &[]).expect("merge");
        verify_type_names(&base, &merged).expect("type names are preserved");
        assert_eq!(
            merged.packages.iter().map(|p| p.type_id_offset).collect::<Vec<_>>(),
            base.packages.iter().map(|p| p.type_id_offset).collect::<Vec<_>>()
        );
    }

    #[test]
    fn shifted_type_offset_is_detected() {
        let base = table(
            &["Hello"],
            typed_package(
                1,
                &["string"],
                &["app_name"],
                vec![string_spec(2), string_type(2, ResConfig::zeroed(), 0)],
            ),
        );
        let mut shifted = base.clone();
        shifted.packages[0].type_id_offset = 0;
        assert!(verify_type_names(&base, &shifted).is_err());
    }
}

fn is_placeholder_type_name(name: &str) -> bool {
    name.starts_with('?')
}
