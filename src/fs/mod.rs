//! Includes the FileSystem trait and built-in implementations.

use std::error::Error;
use std::fs::File;
use std::io::SeekFrom;
use std::io::{Read, Seek};
use std::path::PathBuf;
use std::time::SystemTime;

mod embedded;

/// Implement this trait to provide a filesystem to serve from.
pub trait FileSystem {
    fn is_file<P: ToString>(&self, path: P) -> bool;
    fn last_modified<P: ToString>(&self, path: P) -> Result<SystemTime, Box<Error>>;
    fn size<P: ToString>(&self, path: P) -> Result<u64, Box<Error>>;
    fn open<P: ToString>(&self, path: P, start: Option<u64>) -> Result<Box<Read>, Box<Error>>;
    fn path_valid<P: ToString>(&self, path: P) -> bool;
}

/// Implements the FileSystem trait to handle a local directory.
pub struct LocalFileSystem {
    path: PathBuf,
}

impl LocalFileSystem {
    pub fn new<P: ToString>(path: P) -> LocalFileSystem {
        LocalFileSystem {
            path: PathBuf::from(path.to_string()),
        }
    }
}

impl FileSystem for LocalFileSystem {
    fn is_file<P: ToString>(&self, path: P) -> bool {
        self.path.join(path.to_string()).is_file()
    }

    fn last_modified<P: ToString>(&self, path: P) -> Result<SystemTime, Box<Error>> {
        let modified = self.path.join(path.to_string()).metadata()?.modified()?;
        Ok(modified)
    }

    fn size<P: ToString>(&self, path: P) -> Result<u64, Box<Error>> {
        let len = self.path.join(path.to_string()).metadata()?.len();
        Ok(len)
    }

    fn open<P: ToString>(&self, path: P, start: Option<u64>) -> Result<Box<Read>, Box<Error>> {
        let mut f = File::open(self.path.join(path.to_string()))?;
        if let Some(start) = start {
            f.seek(SeekFrom::Start(start))?;
        }
        Ok(Box::new(f))
    }

    fn path_valid<P: ToString>(&self, path: P) -> bool {
        let path = self.path.join(path.to_string());
        path.starts_with(&self.path)
    }
}
