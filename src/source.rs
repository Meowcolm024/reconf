use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub trait SourceProvider {
    fn load_source(&mut self, path: &Path) -> Result<LoadedSource>;
}

#[derive(Debug, Clone)]
pub struct LoadedSource {
    pub path: PathBuf,
    pub text: String,
}

#[derive(Clone, Default)]
pub struct FilesystemSourceProvider;

impl SourceProvider for FilesystemSourceProvider {
    fn load_source(&mut self, path: &Path) -> Result<LoadedSource> {
        let path = path
            .canonicalize()
            .map_err(|error| Error::new(format!("unknown import `{}`: {error}", path.display())))?;
        let text = fs::read_to_string(&path)
            .map_err(|error| Error::new(format!("unknown import `{}`: {error}", path.display())))?;
        Ok(LoadedSource { path, text })
    }
}

#[derive(Clone, Default)]
pub struct MemorySourceProvider {
    files: BTreeMap<PathBuf, String>,
}

impl MemorySourceProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, path: impl Into<PathBuf>, text: impl Into<String>) {
        self.files
            .insert(normalize_memory_path(path.into()), text.into());
    }

    pub fn with_file(mut self, path: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        self.insert(path, text);
        self
    }
}

impl SourceProvider for MemorySourceProvider {
    fn load_source(&mut self, path: &Path) -> Result<LoadedSource> {
        let path = normalize_memory_path(path);
        let text = self
            .files
            .get(&path)
            .cloned()
            .ok_or_else(|| Error::new(format!("unknown import `{}`", path.display())))?;
        Ok(LoadedSource { path, text })
    }
}

fn normalize_memory_path(path: impl AsRef<Path>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

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

pub struct SourceMap<S = FilesystemSourceProvider> {
    files: Vec<SourceFile>,
    sources: S,
}

impl SourceMap<FilesystemSourceProvider> {
    pub fn new() -> Self {
        Self::with_sources(FilesystemSourceProvider)
    }
}

impl Default for SourceMap<FilesystemSourceProvider> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> SourceMap<S> {
    pub fn with_sources(sources: S) -> Self {
        Self {
            files: Vec::new(),
            sources,
        }
    }

    pub fn get(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(id.0)
    }
}

impl<S: SourceProvider> SourceMap<S> {
    pub fn load(&mut self, path: &Path) -> Result<FileId> {
        let LoadedSource { path, text } = self.sources.load_source(path)?;
        let id = FileId(self.files.len());
        self.files.push(SourceFile { id, path, text });
        Ok(id)
    }
}
