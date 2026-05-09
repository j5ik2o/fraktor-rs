#[cfg(test)]
mod tests;

extern crate std;

use core::time::Duration;
use std::{
  io::{Error, ErrorKind, Result as IoResult, Write},
  sync::mpsc::{SyncSender, TrySendError},
  thread,
  time::Instant,
  vec::Vec,
};

/// Polling interval used while waiting for the downstream channel to free a
/// slot. Small enough to honour sub-second `write_timeout` values without
/// excessive CPU usage.
const WRITE_POLL_INTERVAL: Duration = Duration::from_millis(1);

/// Materialized write-end of the channel used by
/// [`StreamConverters::as_output_stream`](super::StreamConverters::as_output_stream).
///
/// Pekko parity: the blocking `java.io.OutputStream` materialized by
/// `StreamConverters.asOutputStream`. Each call to [`Write::write`] copies the
/// caller's buffer into a freshly allocated `Vec<u8>` and enqueues it onto the
/// internal `sync_channel` (blocking up to the configured `write_timeout`).
/// When the downstream stage drops its receiver end, subsequent writes return
/// [`ErrorKind::BrokenPipe`]. [`Write::flush`] is a no-op because
/// `sync_channel` provides no intermediate buffer beyond the channel itself.
pub struct StreamOutputStream {
  sender:        SyncSender<Vec<u8>>,
  write_timeout: Duration,
}

impl StreamOutputStream {
  /// Creates a new writer backed by the given channel sender.
  ///
  /// `write_timeout` bounds how long [`Write::write`] may block waiting for
  /// the downstream to drain a slot before returning [`ErrorKind::TimedOut`].
  pub(crate) const fn from_channel(sender: SyncSender<Vec<u8>>, write_timeout: Duration) -> Self {
    Self { sender, write_timeout }
  }
}

impl Write for StreamOutputStream {
  fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
    if buf.is_empty() {
      return Ok(0);
    }
    let deadline = Instant::now().checked_add(self.write_timeout);
    let mut payload = buf.to_vec();
    loop {
      match self.sender.try_send(payload) {
        | Ok(()) => return Ok(buf.len()),
        | Err(TrySendError::Disconnected(_)) => return Err(Error::from(ErrorKind::BrokenPipe)),
        | Err(TrySendError::Full(returned)) => {
          match deadline {
            | Some(deadline) if Instant::now() < deadline => {},
            | Some(_) => return Err(Error::from(ErrorKind::TimedOut)),
            | None => {
              // Duration overflow: treat as effectively-infinite wait.
            },
          }
          payload = returned;
          thread::sleep(WRITE_POLL_INTERVAL);
        },
      }
    }
  }

  fn flush(&mut self) -> IoResult<()> {
    Ok(())
  }
}
