use std::{
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

pub mod app;
pub mod auth;
pub mod clients;
pub mod handlers;
pub mod models;
pub mod repositories;
pub mod services;
pub mod shared;
pub mod views;

#[derive(Debug, Clone)]
pub struct TempDir {
    inner: PathBuf,
}

impl TempDir {
    pub fn create() -> Result<Self, std::io::Error> {
        let dir = std::env::temp_dir().join(format!("tempdir_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self { inner: dir })
    }

    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.inner
    }
}

impl Deref for TempDir {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for TempDir {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if self.inner.exists() {
            // should we really unwrap here?
            std::fs::remove_dir_all(&self.inner).expect("Failed to remove temp dir");
        }
    }
}

#[derive(Debug, Clone)]
pub struct TempFile {
    inner: PathBuf,
}

impl TempFile {
    pub fn create(contents: &str, path: &Path) -> Result<Self, std::io::Error> {
        let file_path = path.join(format!("tempfile_{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(&file_path, contents)?;
        Ok(Self { inner: file_path })
    }

    pub fn create_named(name: &str, contents: &str, path: PathBuf) -> Result<Self, std::io::Error> {
        let file_path = path.join(name);
        std::fs::write(&file_path, contents)?;
        Ok(Self { inner: file_path })
    }

    #[must_use]
    pub fn path(&self) -> &PathBuf {
        &self.inner
    }
}

impl Deref for TempFile {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl DerefMut for TempFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if self.inner.exists() {
            std::fs::remove_file(&self.inner).expect("Failed to remove temp file");
        }
    }
}
