// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{self, Read, Write};

use crate::io_error_kind::IOErrorKind;

pub type ReplayIOError = IOErrorKind;
pub type ReplayIOResult<T> = Result<T, ReplayIOError>;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Default, Clone)]
pub struct RecordedStream {
    pub reads: VecDeque<ReplayIOResult<Vec<u8>>>,
    pub writes: VecDeque<ReplayIOResult<Vec<u8>>>,
    pub flushes: VecDeque<ReplayIOResult<()>>,
}

impl RecordedStream {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Read for RecordedStream {
    fn read(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
        if self.reads.is_empty() {
            // shouldn't happen if state machine is exactly the same.
            return Ok(0);
        }

        if self.reads[0].is_err() {
            return Err(io::Error::new(
                self.reads.pop_front().unwrap().err().unwrap().into(),
                "replay",
            ));
        }

        let bytes = self.reads[0].as_mut().unwrap();
        let read_len = buf.write(bytes)?;

        if bytes.len() > read_len {
            // shouldn't happen if state machine is exactly the same.
            bytes.drain(0..buf.len());
        } else {
            self.reads.pop_front();
        }

        Ok(read_len)
    }
}

impl Write for RecordedStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.writes.is_empty() {
            // shouldn't happen if state machine is exactly the same.
            return Ok(0);
        }

        if self.writes[0].is_err() {
            return Err(io::Error::new(
                self.writes.pop_front().unwrap().err().unwrap().into(),
                "replay",
            ));
        }

        let bytes = self.writes[0].as_mut().unwrap();
        let written_len = bytes.len().min(buf.len());

        assert_eq!(&bytes[0..written_len], &buf[0..written_len]);

        if written_len < bytes.len() {
            // shouldn't happen if state machine is exactly the same.
            bytes.drain(0..written_len);
        } else {
            self.writes.pop_front();
        }

        Ok(written_len)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes
            .pop_front()
            .unwrap_or(Ok(()))
            .map_err(|err_kind| io::Error::new(err_kind.into(), "replay"))
    }
}

pub struct StreamRecorder<S> {
    stream: S,
    recorded: RecordedStream,
}

impl<S> StreamRecorder<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            recorded: Default::default(),
        }
    }

    pub fn record(&mut self) -> &mut Self {
        self
    }

    pub fn finish_recording(self) -> RecordedStream {
        self.recorded
    }
}

impl<S> Read for StreamRecorder<S>
where
    S: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream
            .read(buf)
            .map(|len| {
                self.recorded.reads.push_back(Ok(buf[0..len].to_vec()));
                len
            })
            .map_err(|err| {
                self.recorded.reads.push_back(Err(err.kind().into()));
                err
            })
    }
}

impl<S> Write for StreamRecorder<S>
where
    S: Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream
            .write(buf)
            .map(|len| {
                self.recorded.writes.push_back(Ok(buf[0..len].to_vec()));
                len
            })
            .map_err(|err| {
                self.recorded.writes.push_back(Err(err.kind().into()));
                err
            })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream
            .flush()
            .map(|_| {
                self.recorded.flushes.push_back(Ok(()).into());
            })
            .map_err(|err| {
                self.recorded.flushes.push_back(Err(err.kind().into()));
                err
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recording_recorded_is_same() {
        let old_recorded = RecordedStream {
            reads: vec![
                Ok(vec![1, 2, 3, 4]),
                Ok(vec![5, 6, 7]),
                Err(ReplayIOError::WouldBlock),
            ]
            .into(),
            writes: vec![
                Ok(vec![1, 2, 3, 4]),
                Ok(vec![5, 6, 7]),
                Err(ReplayIOError::WouldBlock),
            ]
            .into(),
            flushes: vec![Ok(()), Err(ReplayIOError::WouldBlock)].into(),
        };
        let mut input = old_recorded.clone();

        let new_recorded = {
            let mut recorder = StreamRecorder::new(&mut input);
            let recording = recorder.record();

            // reads
            let mut read = vec![0; 16];
            let len = 4;
            assert_eq!(recording.read(&mut read).unwrap(), len);
            assert_eq!(&read[..len], old_recorded.reads[0].as_ref().unwrap());

            let mut read = vec![0; 16];
            let len = 3;
            assert_eq!(recording.read(&mut read).unwrap(), len);
            assert_eq!(&read[..len], old_recorded.reads[1].as_ref().unwrap());

            let err = recording.read(&mut vec![0; 16]).map_err(|err| err.kind());
            assert!(matches!(err, Err(io::ErrorKind::WouldBlock)));

            // writes
            assert_eq!(recording.write(&[1, 2, 3, 4]).unwrap(), 4);
            assert_eq!(recording.write(&[5, 6, 7]).unwrap(), 3);

            let err = recording.write(&[0, 0, 0]).map_err(|err| err.kind());
            assert!(matches!(err, Err(io::ErrorKind::WouldBlock)));

            // flushes
            recording
                .flush()
                .expect("by definition first flush should have succeeded");
            let err = recording.flush().map_err(|err| err.kind());
            assert!(matches!(err, Err(io::ErrorKind::WouldBlock)));

            // make sure everything is finished/consumed
            assert_eq!(recorder.stream.reads.len(), 0);
            assert_eq!(recorder.stream.writes.len(), 0);
            assert_eq!(recorder.stream.flushes.len(), 0);

            recorder.finish_recording()
        };

        assert_eq!(old_recorded, new_recorded);
    }
}
