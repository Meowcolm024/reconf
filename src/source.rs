use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub file: FileId,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: FileId,
    pub path: PathBuf,
    pub text: String,
}

#[derive(Default)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

impl SourceMap {
    pub fn load(&mut self, path: &Path) -> Result<FileId> {
        let path = path
            .canonicalize()
            .map_err(|error| Error::new(format!("unknown import `{}`: {error}", path.display())))?;
        let text = fs::read_to_string(&path)
            .map_err(|error| Error::new(format!("unknown import `{}`: {error}", path.display())))?;
        let id = FileId(self.files.len());
        self.files.push(SourceFile { id, path, text });
        Ok(id)
    }

    pub fn get(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0)
    }
}
