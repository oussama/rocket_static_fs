//! Includes the FileSystem trait and built-in implementations.

use std::error::Error;
use std::fs::File;
use std::io::SeekFrom;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

mod embedded;

pub use self::embedded::create_package_from_dir;
pub use self::embedded::write_package;
pub use self::embedded::EmbeddedFileSystem;

/// Implement this trait to provide a filesystem to serve from.
pub trait FileSystem {
    fn is_file<P: AsRef<Path>>(&self, path: P) -> bool;
    fn last_modified<P: AsRef<Path>>(&self, path: P) -> Result<SystemTime, Box<Error>>;
    fn size<P: AsRef<Path>>(&self, path: P) -> Result<u64, Box<Error>>;
    fn open<P: AsRef<Path>>(&self, path: P, start: Option<u64>) -> Result<Box<Read>, Box<Error>>;
    fn path_valid<P: AsRef<Path>>(&self, path: P) -> bool;
}

/// Implements the FileSystem trait to handle a local directory.
pub struct LocalFileSystem {
    path: PathBuf,
}

impl LocalFileSystem {
    pub fn new<P: AsRef<Path>>(path: P) -> LocalFileSystem {
        LocalFileSystem {
            path: path.as_ref().to_owned(),
        }
    }
}

impl FileSystem for LocalFileSystem {
    fn is_file<P: AsRef<Path>>(&self, path: P) -> bool {
        self.path.join(path).is_file()
    }

    fn last_modified<P: AsRef<Path>>(&self, path: P) -> Result<SystemTime, Box<Error>> {
        let modified = self.path.join(path).metadata()?.modified()?;
        Ok(modified)
    }

    fn size<P: AsRef<Path>>(&self, path: P) -> Result<u64, Box<Error>> {
        let len = self.path.join(path).metadata()?.len();
        Ok(len)
    }

    fn open<P: AsRef<Path>>(&self, path: P, start: Option<u64>) -> Result<Box<Read>, Box<Error>> {
        let mut f = File::open(self.path.join(path))?;
        if let Some(start) = start {
            f.seek(SeekFrom::Start(start))?;
        }
        Ok(Box::new(f))
    }

    fn path_valid<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = self.path.join(path);
        path.starts_with(&self.path)
    }
}
