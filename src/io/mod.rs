use std::io::{self, Read};

/// A simple limiting reader. It will read at most n bytes from the underlying stream.
///
/// Since there is no internal buffer, you can can safely use LimitReader::into_inner
/// after you are done with it.
pub struct LimitReader<T>
where
    T: Read,
{
    read: u64,
    limit: u64,
    inner: T,
}

impl<T> LimitReader<T>
where
    T: Read,
{
    pub fn new(inner: T, limit: u64) -> Self {
        LimitReader {
            inner,
            limit,
            read: 0,
        }
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> Read for LimitReader<T>
where
    T: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.limit == self.read {
            return Ok(0);
        }

        let left = (self.limit - self.read) as usize;

        let capacity = if left < buf.len() {
            left as usize
        } else {
            buf.len()
        };

        let mut buffer = vec![0u8; capacity];

        let read = self.inner.read(buffer.as_mut_slice())?;
        self.read += read as u64;

        if buf.len() == buffer.len() {
            buf.copy_from_slice(&buffer);
        } else {
            for (i, b) in buffer.iter().enumerate() {
                buf[i] = *b;
            }
        }
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::LimitReader;
    use std::io::Read;

    #[test]
    fn test_limit_reader() {
        let data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        let cursor = Cursor::new(data);

        let mut limit_reader = LimitReader::new(cursor, 6);

        {
            let buffer = &mut [0u8; 4];
            limit_reader.read(buffer).unwrap();
            assert_eq!(buffer, &[0, 1, 2, 3]);
        }

        {
            let buffer = &mut [0u8; 4];
            limit_reader.read(buffer).unwrap();
            assert_eq!(buffer, &[4, 5, 0, 0]);
        }
    }

    use std::io::Cursor;
}
