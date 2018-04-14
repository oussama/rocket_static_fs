use byteorder::{BigEndian, ReadBytesExt};
use chrono::{DateTime, TimeZone, Utc};
use fs::FileSystem;
use std::collections::HashMap;
use std::error::Error;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::str::FromStr;
use std::time::SystemTime;

/// Provides a FileSystem which is embedded in the binary.
struct EmbeddedFileSystem<'a> {
    package: Package<'a>,
}

impl<'a> FileSystem for EmbeddedFileSystem<'a> {
    fn is_file<P: ToString>(&self, path: P) -> bool {
        self.package.files.contains_key(&path.to_string())
    }

    fn last_modified<P: ToString>(&self, path: P) -> Result<SystemTime, Box<Error>> {
        match self.package.files.get(&path.to_string()) {
            Some(file) => Ok(file.last_modified.into()),
            None => Err(Box::new(::Error::new("file does not exist"))),
        }
    }

    fn size<P: ToString>(&self, path: P) -> Result<u64, Box<Error>> {
        match self.package.files.get(&path.to_string()) {
            Some(file) => Ok(file.len),
            None => Err(Box::new(::Error::new("file does not exist"))),
        }
    }

    fn open<P: ToString>(&self, path: P, start: Option<u64>) -> Result<Box<Read>, Box<Error>> {
        match self.package.files.get(&path.to_string()) {
            Some(file) => {
                let start = file.start as usize;
                let end = (file.start + file.len - 1) as usize;
                let slice = &self.package.data[start..end];
                Ok(Box::new(Cursor::new(slice)))
            }
            None => Err(Box::new(::Error::new("file does not exist"))),
        }
    }

    fn path_valid<P: ToString>(&self, path: P) -> bool {
        self.package.files.contains_key(&path.to_string())
    }
}

struct Package<'a> {
    files: HashMap<String, File>,
    data: &'a [u8],
}

struct File {
    last_modified: DateTime<Utc>,
    len: u64,
    start: u64,
}

impl<'a> Package<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, Box<Error>> {
        let mut cursor = Cursor::new(bytes);
        let meta_len = cursor.read_u64::<BigEndian>()?;

        let mut files = HashMap::new();
        let mut read = 0;

        while read < meta_len {
            let cursor_start = cursor.position();
            let path_len = cursor.read_u16::<BigEndian>()? as u64;
            let mut path = String::new();
            let cursor_clone = cursor.clone();
            let mut path_reader = cursor_clone.take(path_len);
            path_reader.read_to_string(&mut path)?;
            cursor.seek(SeekFrom::Current(path_len as i64))?;

            let last_modified_seconds = cursor.read_i64::<BigEndian>()?;
            let last_modified: DateTime<Utc> = Utc.timestamp(last_modified_seconds, 0);

            let len = cursor.read_u64::<BigEndian>()?;
            let start = cursor.read_u64::<BigEndian>()?;

            let cursor_end = cursor.position();

            read += cursor_end - cursor_start;

            files.insert(
                path,
                File {
                    last_modified,
                    len,
                    start,
                },
            );
        }

        let data = &bytes[meta_len as usize..];
        Ok(Package { files, data })
    }
}

fn write_package<T>(root: &str, input_files: &[&str], writer: T)
where
    T: Write,
{
    let mut files = Vec::from(input_files);
    files.sort();
    unimplemented!()
}
