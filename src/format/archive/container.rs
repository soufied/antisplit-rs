use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, Write};
use std::path::Path;

use zip::ZipArchive;

use crate::error::Result;

#[derive(Clone, Debug)]
pub struct EntryInfo {
    pub name: String,
    pub size: u64,
    pub crc: u32,
}

pub trait Container {
    fn entries(&self) -> &[EntryInfo];

    fn read(&mut self, name: &str) -> Result<Vec<u8>>;

    fn extract_to(&mut self, name: &str, destination: &Path) -> Result<u64> {
        let bytes = self.read(name)?;
        fs::write(destination, &bytes)?;
        Ok(bytes.len() as u64)
    }
}

pub struct ZipContainer<R: Read + Seek> {
    archive: ZipArchive<R>,
    entries: Vec<EntryInfo>,
}

impl<R: Read + Seek> ZipContainer<R> {
    pub fn new(reader: R) -> Result<Self> {
        let mut archive = ZipArchive::new(reader)?;
        let mut entries = Vec::with_capacity(archive.len());
        for index in 0..archive.len() {
            let file = archive.by_index(index)?;
            if file.is_dir() {
                continue;
            }
            entries.push(EntryInfo {
                name: file.name().to_owned(),
                size: file.size(),
                crc: file.crc32(),
            });
        }
        Ok(Self { archive, entries })
    }
}

impl<R: Read + Seek> Container for ZipContainer<R> {
    fn entries(&self) -> &[EntryInfo] {
        &self.entries
    }

    fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let mut file = self.archive.by_name(name)?;
        let mut buffer = Vec::with_capacity(usize::try_from(file.size()).unwrap_or(0));
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }

    fn extract_to(&mut self, name: &str, destination: &Path) -> Result<u64> {
        let mut file = self.archive.by_name(name)?;
        let mut output = File::create(destination)?;
        Ok(io::copy(&mut file, &mut output)?)
    }
}

impl<R: Read + Seek> ZipContainer<R> {
    pub fn stream_to<W: Write>(&mut self, name: &str, writer: &mut W) -> Result<u64> {
        let mut file = self.archive.by_name(name)?;
        Ok(io::copy(&mut file, writer)?)
    }
}

pub fn open_file(path: &Path) -> Result<ZipContainer<BufReader<File>>> {
    let file = File::open(path)?;
    ZipContainer::new(BufReader::new(file))
}
