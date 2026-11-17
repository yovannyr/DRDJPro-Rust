//! Hard logical-file bound, including random-access header rewrites.
use std::io::{self, Seek, SeekFrom, Write};

pub(crate) struct LimitedWriter<W> {
    inner: W,
    limit: u64,
    position: u64,
    failed: bool,
}
impl<W: Write + Seek> LimitedWriter<W> {
    pub(crate) fn new(mut inner: W, limit: u64) -> io::Result<Self> {
        let position = inner.stream_position()?;
        let end = inner.seek(SeekFrom::End(0))?;
        inner.seek(SeekFrom::Start(position))?;
        if position > limit || end > limit {
            return Err(Self::error());
        }
        Ok(Self {
            inner,
            limit,
            position,
            failed: false,
        })
    }
    fn error() -> io::Error {
        io::Error::other("Minidump write failed or exceeded its file limit")
    }
    pub(crate) fn check(&self) -> io::Result<()> {
        if self.failed {
            Err(Self::error())
        } else {
            Ok(())
        }
    }
    fn fail<T>(&mut self) -> io::Result<T> {
        self.failed = true;
        Err(Self::error())
    }
}
impl<W: Write + Seek> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.check()?;
        if self
            .position
            .checked_add(bytes.len() as u64)
            .is_none_or(|end| end > self.limit)
        {
            return self.fail();
        }
        match self.inner.write(bytes) {
            Ok(0) if !bytes.is_empty() => self.fail(),
            Ok(n) => {
                self.position += n as u64;
                Ok(n)
            }
            Err(e) => {
                self.failed |= e.kind() != io::ErrorKind::Interrupted;
                Err(e)
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        self.check()?;
        self.inner
            .flush()
            .inspect_err(|e| self.failed |= e.kind() != io::ErrorKind::Interrupted)
    }
}
impl<W: Write + Seek> Seek for LimitedWriter<W> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.check()?;
        let target = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => i128::from(self.position) + i128::from(n),
            SeekFrom::End(n) => {
                let end = self
                    .inner
                    .seek(SeekFrom::End(0))
                    .inspect_err(|e| self.failed |= e.kind() != io::ErrorKind::Interrupted)?;
                i128::from(end) + i128::from(n)
            }
        };
        if target < 0 || target > i128::from(self.limit) {
            return self.fail();
        }
        self.position = self
            .inner
            .seek(SeekFrom::Start(target as u64))
            .inspect_err(|e| self.failed |= e.kind() != io::ErrorKind::Interrupted)?;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn exact_boundary_and_rewrites_are_not_a_cumulative_quota() {
        let mut writer = LimitedWriter::new(Cursor::new(Vec::new()), 8).unwrap();
        writer.write_all(b"12345678").unwrap();
        for _ in 0..10 {
            writer.seek(SeekFrom::Start(0)).unwrap();
            writer.write_all(b"HEADER").unwrap();
        }
        writer.seek(SeekFrom::End(0)).unwrap();
        writer.write_all(b"").unwrap();
        writer.check().unwrap();
        assert_eq!(writer.inner.get_ref(), b"HEADER78");
    }
    #[test]
    fn crossing_write_is_rejected_before_any_bytes_and_failure_is_sticky() {
        let mut writer = LimitedWriter::new(Cursor::new(Vec::new()), 8).unwrap();
        writer.write_all(b"1234").unwrap();
        assert!(writer.write_all(b"56789").is_err());
        assert_eq!(writer.inner.get_ref(), b"1234");
        assert!(writer.seek(SeekFrom::Start(0)).is_err());
        assert!(writer.write_all(b"x").is_err());
        assert!(writer.check().is_err());
    }
    #[test]
    fn sparse_seek_overflow_and_negative_offsets_cannot_extend_file() {
        for from in [
            SeekFrom::Start(u64::MAX),
            SeekFrom::Current(i64::MAX),
            SeekFrom::End(-1),
        ] {
            let mut writer = LimitedWriter::new(Cursor::new(Vec::new()), 8).unwrap();
            assert!(writer.seek(from).is_err());
            assert!(writer.inner.get_ref().is_empty());
        }
        let mut writer = LimitedWriter::new(Cursor::new(Vec::new()), 8).unwrap();
        writer.seek(SeekFrom::Start(8)).unwrap();
        assert!(writer.write_all(b"x").is_err());
        assert!(writer.inner.get_ref().is_empty());
    }
    #[test]
    fn existing_oversized_file_is_rejected() {
        assert!(LimitedWriter::new(Cursor::new(vec![0; 9]), 8).is_err());
    }

    #[test]
    fn interrupted_writes_retry_but_zero_and_storage_errors_remain_failures() {
        struct Fault {
            data: Cursor<Vec<u8>>,
            kind: io::ErrorKind,
            first: bool,
        }
        impl Write for Fault {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.first {
                    self.first = false;
                    if self.kind == io::ErrorKind::WriteZero {
                        return Ok(0);
                    }
                    return Err(self.kind.into());
                }
                self.data.write(bytes)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        impl Seek for Fault {
            fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
                self.data.seek(position)
            }
        }
        for kind in [
            io::ErrorKind::Interrupted,
            io::ErrorKind::StorageFull,
            io::ErrorKind::WriteZero,
        ] {
            let inner = Fault {
                data: Cursor::new(Vec::new()),
                kind,
                first: true,
            };
            let mut writer = LimitedWriter::new(inner, 8).unwrap();
            if kind == io::ErrorKind::Interrupted {
                writer.write_all(b"data").unwrap();
                writer.check().unwrap();
                assert_eq!(writer.inner.data.get_ref(), b"data");
            } else {
                assert!(writer.write_all(b"data").is_err());
                assert!(writer.check().is_err());
                assert!(writer.write_all(b"x").is_err());
                assert!(writer.inner.data.get_ref().is_empty());
            }
        }
    }
}
