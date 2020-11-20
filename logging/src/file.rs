// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fs;
use std::fs::{File, OpenOptions};
use std::io;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use libflate::gzip::Encoder as GzipEncoder;

#[derive(Debug)]
pub struct FileAppenderBuilder {
    appender: FileAppender,
}

impl FileAppenderBuilder {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        FileAppenderBuilder {
            appender: FileAppender::new(path),
        }
    }

    /// By default, logger just appends log messages to file.
    /// If this method called, logger truncates the file to 0 length when opening.
    pub fn truncate(mut self) -> Self {
        self.appender.truncate = true;
        self
    }

    /// Sets the threshold used for determining whether rotate the current log file.
    ///
    /// If the byte size of the current log file exceeds this value, the file will be rotated.
    /// The name of the rotated file will be `"${ORIGINAL_FILE_NAME}.0"`.
    /// If there is a previously rotated file,
    /// it will be renamed to `"${ORIGINAL_FILE_NAME}.1"` before rotation of the current log file.
    /// This process is iterated recursively until log file names no longer conflict or
    /// [`rotate_keep`] limit reached.
    ///
    /// The default value is `std::u64::MAX`.
    ///
    /// [`rotate_keep`]: ./struct.FileLoggerBuilder.html#method.rotate_keep
    pub fn rotate_size(mut self, size: u64) -> Self {
        self.appender.rotate_size = size;
        self
    }

    /// Sets the maximum number of rotated log files to keep.
    ///
    /// If the number of rotated log files exceed this value, the oldest log file will be deleted.
    ///
    /// The default value is `8`.
    pub fn rotate_keep(mut self, count: usize) -> Self {
        self.appender.rotate_keep = count;
        self
    }

    /// Sets whether to compress or not compress rotated files.
    ///
    /// If `true` is specified, rotated files will be compressed by GZIP algorithm and
    /// the suffix ".gz" will be appended to those file names.
    ///
    /// The default value is `false`.
    pub fn rotate_compress(mut self, compress: bool) -> Self {
        self.appender.rotate_compress = compress;
        self
    }

    pub fn build(self) -> FileAppender {
        self.appender
    }
}

#[derive(Debug)]
pub struct FileAppender {
    path: PathBuf,
    file: Option<BufWriter<File>>,
    truncate: bool,
    written_size: u64,
    rotate_size: u64,
    rotate_keep: usize,
    rotate_compress: bool,
    wait_compression: Option<mpsc::Receiver<io::Result<()>>>,
    next_reopen_check: Instant,
    reopen_check_interval: Duration,
}

impl Clone for FileAppender {
    fn clone(&self) -> Self {
        FileAppender {
            path: self.path.clone(),
            file: None,
            truncate: self.truncate,
            written_size: 0,
            rotate_size: self.rotate_size,
            rotate_keep: self.rotate_keep,
            rotate_compress: self.rotate_compress,
            wait_compression: None,
            next_reopen_check: Instant::now(),
            reopen_check_interval: self.reopen_check_interval,
        }
    }
}

impl FileAppender {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        FileAppender {
            path: path.as_ref().to_path_buf(),
            file: None,
            truncate: false,
            written_size: 0,
            rotate_size: default_rotate_size(),
            rotate_keep: default_rotate_keep(),
            rotate_compress: false,
            wait_compression: None,
            next_reopen_check: Instant::now(),
            reopen_check_interval: Duration::from_millis(1000),
        }
    }

    fn reopen_if_needed(&mut self) -> io::Result<()> {
        // See issue #18
        // Basically, path.exists() is VERY slow on windows, so we just
        // can't check on every write. Limit checking to a predefined interval.
        // This shouldn't create problems neither for users, nor for logrotate et al.,
        // as explained in the issue.
        let now = Instant::now();
        let path_exists = if now >= self.next_reopen_check {
            self.next_reopen_check = now + self.reopen_check_interval;
            self.path.exists()
        } else {
            // Pretend the path exists without any actual checking.
            true
        };

        if self.file.is_none() || !path_exists {
            let mut file_builder = OpenOptions::new();
            file_builder.create(true);
            if self.truncate {
                file_builder.truncate(true);
            }
            // If the old file was externally deleted and we attempt to open a new one before releasing the old
            // handle, we get a Permission denied on Windows. Release the handle.
            self.file = None;
            let file = file_builder
                .append(!self.truncate)
                .write(true)
                .open(&self.path)?;
            self.written_size = file.metadata()?.len();
            self.file = Some(BufWriter::new(file));
        }
        Ok(())
    }

    fn rotate(&mut self) -> io::Result<()> {
        if let Some(ref mut rx) = self.wait_compression {
            use std::sync::mpsc::TryRecvError;
            match rx.try_recv() {
                Err(TryRecvError::Empty) => {
                    // The previous compression is in progress
                    return Ok(());
                }
                Err(TryRecvError::Disconnected) => {
                    let e =
                        io::Error::new(io::ErrorKind::Other, "Log file compression thread aborted");
                    return Err(e);
                }
                Ok(result) => {
                    result?;
                }
            }
        }
        self.wait_compression = None;

        let _ = self.file.take();

        for i in (1..=self.rotate_keep).rev() {
            let from = self.rotated_path(i)?;
            let to = self.rotated_path(i + 1)?;
            if from.exists() {
                fs::rename(from, to)?;
            }
        }
        if self.path.exists() {
            let rotated_path = self.rotated_path(1)?;
            if self.rotate_compress {
                let (plain_path, temp_gz_path) = self.rotated_paths_for_compression()?;
                let (tx, rx) = mpsc::channel();

                fs::rename(&self.path, &plain_path)?;
                thread::spawn(move || {
                    let result = Self::compress(plain_path, temp_gz_path, rotated_path);
                    let _ = tx.send(result);
                });

                self.wait_compression = Some(rx);
            } else {
                fs::rename(&self.path, rotated_path)?;
            }
        }

        let delete_path = self.rotated_path(self.rotate_keep + 1)?;
        if delete_path.exists() {
            fs::remove_file(delete_path)?;
        }

        self.written_size = 0;
        self.next_reopen_check = Instant::now();
        self.reopen_if_needed()?;

        Ok(())
    }

    fn rotated_path(&self, i: usize) -> io::Result<PathBuf> {
        let path = self.path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Non UTF-8 log file path: {:?}", self.path),
            )
        })?;
        if self.rotate_compress {
            Ok(PathBuf::from(format!("{}.{}.gz", path, i)))
        } else {
            Ok(PathBuf::from(format!("{}.{}", path, i)))
        }
    }

    fn rotated_paths_for_compression(&self) -> io::Result<(PathBuf, PathBuf)> {
        let path = self.path.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Non UTF-8 log file path: {:?}", self.path),
            )
        })?;
        Ok((
            PathBuf::from(format!("{}.1", path)),
            PathBuf::from(format!("{}.1.gz.temp", path)),
        ))
    }

    fn compress(input_path: PathBuf, temp_path: PathBuf, output_path: PathBuf) -> io::Result<()> {
        let mut input = File::open(&input_path)?;
        let mut temp = GzipEncoder::new(File::create(&temp_path)?)?;
        io::copy(&mut input, &mut temp)?;
        temp.finish().into_result()?;

        fs::rename(temp_path, output_path)?;
        fs::remove_file(input_path)?;
        Ok(())
    }
}

impl Write for FileAppender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.reopen_if_needed()?;
        let size = if let Some(ref mut f) = self.file {
            f.write(buf)?
        } else {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Cannot open file: {:?}", self.path),
            ));
        };

        self.written_size += size as u64;
        Ok(size)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(ref mut f) = self.file {
            f.flush()?;
        }
        if self.written_size >= self.rotate_size {
            self.rotate()?;
        }
        Ok(())
    }
}

fn default_rotate_size() -> u64 {
    use std::u64;

    u64::MAX
}

fn default_rotate_keep() -> usize {
    8
}

#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use tempfile::{Builder as TempDirBuilder, TempDir};

    use super::*;

    macro_rules! write (
        ($l:expr, $t:expr) => {
            $l.write_all($t.as_bytes()).unwrap();
            $l.flush().unwrap();
        };
    );

    #[test]
    fn file_rotation_works() {
        let dir = tempdir();
        let mut appender = FileAppenderBuilder::new(dir.path().join("foo.log"))
            .rotate_size(128)
            .rotate_keep(2)
            .build();

        write!(appender, "hello");
        thread::sleep(Duration::from_millis(50));
        assert!(dir.path().join("foo.log").exists());
        assert!(!dir.path().join("foo.log.1").exists());

        write!(appender, "world");
        thread::sleep(Duration::from_millis(50));
        assert!(dir.path().join("foo.log").exists());
        assert!(!dir.path().join("foo.log.1").exists());
        assert!(!dir.path().join("foo.log.2").exists());
        assert!(!dir.path().join("foo.log.3").exists());

        write!(appender, format!("vec(0): {:?}", vec![0; 128]));
        thread::sleep(Duration::from_millis(50));
        assert!(dir.path().join("foo.log").exists());
        assert!(dir.path().join("foo.log.1").exists());
        assert!(!dir.path().join("foo.log.2").exists());
        assert!(!dir.path().join("foo.log.3").exists());

        write!(appender, format!("vec(1): {:?}", vec![0; 128]));
        thread::sleep(Duration::from_millis(50));
        assert!(dir.path().join("foo.log").exists());
        assert!(dir.path().join("foo.log.1").exists());
        assert!(dir.path().join("foo.log.2").exists());
        assert!(!dir.path().join("foo.log.3").exists());
    }

    #[test]
    fn file_gzip_rotation_works() {
        let dir = tempdir();
        let mut appender = FileAppenderBuilder::new(dir.path().join("foo.log"))
            .rotate_size(128)
            .rotate_keep(2)
            .rotate_compress(true)
            .build();

        write!(appender, "hello");
        thread::sleep(Duration::from_millis(50));
        assert!(dir.path().join("foo.log").exists());
        assert!(!dir.path().join("foo.log.1").exists());

        write!(appender, "world");
        thread::sleep(Duration::from_millis(50));
        assert!(dir.path().join("foo.log").exists());
        assert!(!dir.path().join("foo.log.1.gz").exists());
        assert!(!dir.path().join("foo.log.2.gz").exists());

        write!(appender, format!("vec(0): {:?}", vec![0; 128]));
        thread::sleep(Duration::from_millis(50));
        assert!(dir.path().join("foo.log").exists());
        assert!(dir.path().join("foo.log.1.gz").exists());
        assert!(!dir.path().join("foo.log.2.gz").exists());
        assert!(!dir.path().join("foo.log.3.gz").exists());

        write!(appender, format!("vec(1): {:?}", vec![0; 128]));
        thread::sleep(Duration::from_millis(50));
        assert!(dir.path().join("foo.log").exists());
        assert!(dir.path().join("foo.log.1.gz").exists());
        assert!(dir.path().join("foo.log.2.gz").exists());
        assert!(!dir.path().join("foo.log.3.gz").exists());
    }

    fn tempdir() -> TempDir {
        TempDirBuilder::new()
            .prefix("logging_test")
            .tempdir()
            .expect("Cannot create a temporary directory")
    }
}
