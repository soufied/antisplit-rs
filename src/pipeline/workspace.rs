use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct Workspace {
    root: PathBuf,
    keep: bool,
}

impl Workspace {
    pub fn create() -> io::Result<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "antisplit-rs-{}-{stamp}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("input"))?;
        fs::create_dir_all(root.join("output"))?;
        Ok(Self { root, keep: false })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn input_dir(&self) -> PathBuf {
        self.root.join("input")
    }

    pub fn output_dir(&self) -> PathBuf {
        self.root.join("output")
    }

    pub fn set_keep(&mut self, keep: bool) {
        self.keep = keep;
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        if !self.keep {
            fs::remove_dir_all(&self.root).ok();
        }
    }
}
