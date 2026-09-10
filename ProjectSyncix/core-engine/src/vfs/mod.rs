use std::io::Result;
use std::path::{Path, PathBuf};

/// Sanal Dosya Sistemi (VFS) Provider Arayüzü
/// Bütün file_path okuma/yazma/listeleme işlemleri bu trait üzerinden yapılır.
/// LocalDisk, In-Memory, veya Cloud storage gibi sistemlere genişletilebilir.
pub trait FileSystemProvider: Send + Sync {
    fn read_to_string(&self, path: &Path) -> Result<String>;
    fn write_to_string(&self, path: &Path, contents: &str) -> Result<()>;
    fn exists(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>>;
    fn create_dir_all(&self, path: &Path) -> Result<()>;
    fn remove_file(&self, path: &Path) -> Result<()>;
    fn remove_dir_all(&self, path: &Path) -> Result<()>;
}

/// Standart Local Disk Sağlayıcısı (std::fs)
pub struct LocalDiskProvider;

impl LocalDiskProvider {
    pub fn new() -> Self {
        LocalDiskProvider {}
    }
}

impl FileSystemProvider for LocalDiskProvider {
    fn read_to_string(&self, path: &Path) -> Result<String> {
        std::fs::read_to_string(path)
    }

    fn write_to_string(&self, path: &Path, contents: &str) -> Result<()> {
        std::fs::write(path, contents)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        if path.is_dir() {
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                entries.push(entry.path());
            }
        }
        Ok(entries)
    }

    fn create_dir_all(&self, path: &Path) -> Result<()> {
        std::fs::create_dir_all(path)
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        std::fs::remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> Result<()> {
        std::fs::remove_dir_all(path)
    }
}
