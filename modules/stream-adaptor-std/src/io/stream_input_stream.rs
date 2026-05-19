#[cfg(test)]
#[path = "stream_input_stream_test.rs"]
mod tests;

extern crate std;

use core::time::Duration;
use std::{
  io::{Error, ErrorKind, Read, Result as IoResult},
  sync::mpsc::{Receiver, RecvTimeoutError},
  vec::Vec,
};

/// Materialized read-end of the channel used by
/// [`StreamConverters::as_input_stream`](super::StreamConverters::as_input_stream).
///
/// Pekko parity: the blocking `java.io.InputStream` materialized by
/// `StreamConverters.asInputStream`. Each call to [`Read::read`] pulls one
/// chunk from the internal `sync_channel` (blocking up to the configured
/// `read_timeout`) and copies bytes into the caller's buffer. When a chunk is
/// larger than the caller's buffer, the remainder is retained internally and
/// returned by subsequent reads. End-of-stream is signalled by `Ok(0)` when
/// the upstream sink drops its sender end of the channel.
pub struct StreamInputStream {
  receiver:     Receiver<Vec<u8>>,
  read_timeout: Duration,
  buffer:       Vec<u8>,
}

impl StreamInputStream {
  /// Creates a new reader backed by the given channel receiver.
  ///
  /// `read_timeout` bounds how long [`Read::read`] may block waiting for a new
  /// chunk before returning [`ErrorKind::TimedOut`].
  pub(crate) fn from_channel(receiver: Receiver<Vec<u8>>, read_timeout: Duration) -> Self {
    Self { receiver, read_timeout, buffer: Vec::new() }
  }
}

impl Read for StreamInputStream {
  fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
    if buf.is_empty() {
      return Ok(0);
    }

    if !self.buffer.is_empty() {
      let n = self.buffer.len().min(buf.len());
      buf[..n].copy_from_slice(&self.buffer[..n]);
      self.buffer.drain(..n);
      return Ok(n);
    }

    match self.receiver.recv_timeout(self.read_timeout) {
      | Ok(chunk) => {
        if chunk.is_empty() {
          return Ok(0);
        }
        let n = chunk.len().min(buf.len());
        buf[..n].copy_from_slice(&chunk[..n]);
        if n < chunk.len() {
          self.buffer.extend_from_slice(&chunk[n..]);
        }
        Ok(n)
      },
      | Err(RecvTimeoutError::Timeout) => Err(Error::from(ErrorKind::TimedOut)),
      | Err(RecvTimeoutError::Disconnected) => Ok(0),
    }
  }
}
