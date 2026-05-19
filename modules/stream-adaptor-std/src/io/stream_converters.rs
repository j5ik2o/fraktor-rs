#[cfg(test)]
#[path = "stream_converters_test.rs"]
mod tests;

extern crate std;

use core::time::Duration;
use std::{
  boxed::Box,
  io::{BufReader, BufWriter, Error, ErrorKind, Read, Write},
  sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
  vec::Vec,
};

use fraktor_stream_core_kernel_rs::{
  DemandTracker, DynValue, IOResult, SinkDecision, SinkLogic, SourceLogic, StreamError,
  dsl::{Sink, Source},
  materialization::StreamFuture,
  stage::StageKind,
};

use super::{super::io_error_to_stream_error, StreamInputStream, StreamOutputStream};

/// Internal capacity of the `sync_channel` used by
/// [`StreamConverters::as_input_stream`] / [`StreamConverters::as_output_stream`].
///
/// Mirrors the Pekko default buffer size used by `asInputStream` /
/// `asOutputStream`. 16 slots give enough headroom for small bursts while still
/// keeping memory bounded and propagating backpressure to the materialized
/// reader / writer promptly.
const AS_STREAM_CHANNEL_CAPACITY: usize = 16;

/// Adapters for converting between Rust IO types and stream stages.
///
/// Corresponds to Pekko's `StreamConverters` object. Provides
/// `from_input_stream` and `from_output_stream` factory methods that bridge
/// `std::io::Read` / `std::io::Write` with stream `Source` / `Sink` stages.
pub struct StreamConverters;

impl StreamConverters {
  /// Creates a source that reads bytes from a reader in chunks.
  ///
  /// Pekko parity: `StreamConverters.fromInputStream`. The factory closure is
  /// called lazily on the first pull to obtain the reader. Data is read lazily
  /// on each pull in `chunk_size`-byte chunks and each chunk is emitted as a
  /// `Vec<u8>` element (mirroring Pekko's `ByteString`).
  ///
  /// The materialized value is a [`StreamFuture<IOResult>`] that completes
  /// with the total number of bytes read and the completion status.
  #[must_use]
  pub fn from_input_stream<R, F>(factory: F, chunk_size: usize) -> Source<Vec<u8>, StreamFuture<IOResult>>
  where
    F: FnOnce() -> R + Send + 'static,
    R: Read + Send + 'static, {
    let completion = StreamFuture::new();
    if chunk_size == 0 {
      let error = Error::new(ErrorKind::InvalidInput, "chunk_size must be greater than 0");
      completion.complete(Ok(IOResult::failed(0, io_error_to_stream_error(&error))));
      return Source::empty().map_materialized_value(move |_| completion);
    }

    Source::from_logic(StageKind::Custom, ReaderSourceLogic {
      factory: Some(factory),
      reader: None,
      chunk_size,
      total_bytes: 0,
      completion: completion.clone(),
      done: false,
    })
    .map_materialized_value(move |_| completion)
  }

  /// Creates a sink that writes received byte chunks to a writer.
  ///
  /// Pekko parity: `StreamConverters.fromOutputStream`. The factory closure is
  /// called when the stream starts to obtain the writer. Each received
  /// `Vec<u8>` element (mirroring Pekko's `ByteString`) is written through a
  /// buffered writer. When `auto_flush` is `true`, the writer is flushed after
  /// every chunk. The materialized value is a [`StreamFuture<IOResult>`]
  /// that completes with the total number of bytes written and the completion
  /// status.
  #[must_use]
  pub fn from_output_stream<W, F>(factory: F, auto_flush: bool) -> Sink<Vec<u8>, StreamFuture<IOResult>>
  where
    F: FnOnce() -> W + Send + 'static,
    W: Write + Send + 'static, {
    let completion = StreamFuture::new();
    let logic = WriteToWriterSinkLogic {
      factory: Some(factory),
      auto_flush,
      writer: None,
      count: 0,
      completion: completion.clone(),
    };
    Sink::from_definition(StageKind::Custom, logic, completion)
  }

  /// Creates a sink whose materialized value is a blocking [`std::io::Read`] handle.
  ///
  /// Pekko parity: `StreamConverters.asInputStream`. Each element pushed into
  /// the sink is forwarded into an internal bounded `sync_channel` which the
  /// materialized [`StreamInputStream`] drains on `read`. When the channel is
  /// full the sink defers the element and stops requesting upstream demand
  /// until the reader drains a slot.
  ///
  /// `read_timeout` bounds how long [`std::io::Read::read`] may block waiting
  /// for a new chunk before returning [`std::io::ErrorKind::TimedOut`].
  #[must_use]
  pub fn as_input_stream(read_timeout: Duration) -> Sink<Vec<u8>, StreamInputStream> {
    let (sender, receiver) = sync_channel::<Vec<u8>>(AS_STREAM_CHANNEL_CAPACITY);
    let reader = StreamInputStream::from_channel(receiver, read_timeout);
    let logic = ChannelSinkLogic { sender: Some(sender), pending: None, terminal_deferred: false };
    Sink::from_definition(StageKind::Custom, logic, reader)
  }

  /// Creates a source whose materialized value is a blocking [`std::io::Write`] handle.
  ///
  /// Pekko parity: `StreamConverters.asOutputStream`. Each chunk written to the
  /// materialized [`StreamOutputStream`] is forwarded to the source through an
  /// internal bounded `sync_channel` and emitted as a `Vec<u8>` element. The
  /// source completes when the materialized writer is dropped (channel becomes
  /// disconnected).
  ///
  /// `write_timeout` bounds how long [`std::io::Write::write`] may block when
  /// the channel is full before returning [`std::io::ErrorKind::TimedOut`].
  #[must_use]
  pub fn as_output_stream(write_timeout: Duration) -> Source<Vec<u8>, StreamOutputStream> {
    let (sender, receiver) = sync_channel::<Vec<u8>>(AS_STREAM_CHANNEL_CAPACITY);
    let writer = StreamOutputStream::from_channel(sender, write_timeout);
    let logic = ChannelSourceLogic { receiver };
    Source::from_logic(StageKind::Custom, logic).map_materialized_value(move |_| writer)
  }
}

struct ChannelSinkLogic {
  sender:            Option<SyncSender<Vec<u8>>>,
  pending:           Option<Vec<u8>>,
  /// `true` once upstream has completed or failed while `pending` still held
  /// an element. When the pending element is finally drained, the sender is
  /// dropped so the downstream reader observes EOF.
  terminal_deferred: bool,
}

impl SinkLogic for ChannelSinkLogic {
  fn can_accept_input(&self) -> bool {
    self.pending.is_none() && !self.terminal_deferred && self.sender.is_some()
  }

  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    if self.pending.is_some() {
      return Err(StreamError::WouldBlock);
    }
    let chunk = *input.downcast::<Vec<u8>>().map_err(|_| StreamError::TypeMismatch)?;
    let Some(sender) = self.sender.as_ref() else {
      return Ok(SinkDecision::Complete);
    };
    match sender.try_send(chunk) {
      | Ok(()) => {
        demand.request(1)?;
        Ok(SinkDecision::Continue)
      },
      | Err(TrySendError::Full(rejected)) => {
        self.pending = Some(rejected);
        Ok(SinkDecision::Continue)
      },
      | Err(TrySendError::Disconnected(_)) => {
        self.sender = None;
        Ok(SinkDecision::Complete)
      },
    }
  }

  fn on_tick(&mut self, demand: &mut DemandTracker) -> Result<bool, StreamError> {
    // First try to flush a pending element.
    if let Some(chunk) = self.pending.take() {
      let Some(sender) = self.sender.as_ref() else {
        // Sender already gone; drop pending silently so the sink can terminate.
        self.terminal_deferred = false;
        return Ok(true);
      };
      match sender.try_send(chunk) {
        | Ok(()) => {
          if self.terminal_deferred {
            self.terminal_deferred = false;
            self.sender = None;
          } else {
            demand.request(1)?;
          }
          return Ok(true);
        },
        | Err(TrySendError::Full(rejected)) => {
          self.pending = Some(rejected);
          return Ok(false);
        },
        | Err(TrySendError::Disconnected(_)) => {
          self.sender = None;
          self.terminal_deferred = false;
          return Ok(true);
        },
      }
    }
    // No pending element, but a deferred terminal may still need action.
    if self.terminal_deferred {
      self.terminal_deferred = false;
      self.sender = None;
      return Ok(true);
    }
    Ok(false)
  }

  fn has_pending_work(&self) -> bool {
    self.pending.is_some() || self.terminal_deferred
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    if self.pending.is_some() {
      self.terminal_deferred = true;
    } else {
      self.sender = None;
    }
    Ok(())
  }

  fn on_error(&mut self, _error: StreamError) {
    if self.pending.is_some() {
      self.terminal_deferred = true;
    } else {
      self.sender = None;
    }
  }
}

struct ChannelSourceLogic {
  receiver: Receiver<Vec<u8>>,
}

impl SourceLogic for ChannelSourceLogic {
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    match self.receiver.try_recv() {
      | Ok(chunk) => Ok(Some(Box::new(chunk))),
      | Err(TryRecvError::Empty) => Err(StreamError::WouldBlock),
      | Err(TryRecvError::Disconnected) => Ok(None),
    }
  }
}

struct WriteToWriterSinkLogic<F, W>
where
  W: Write + Send + 'static, {
  factory:    Option<F>,
  auto_flush: bool,
  writer:     Option<BufWriter<W>>,
  count:      u64,
  completion: StreamFuture<IOResult>,
}

struct ReaderSourceLogic<F, R>
where
  R: Read + Send + 'static, {
  factory:     Option<F>,
  reader:      Option<BufReader<R>>,
  chunk_size:  usize,
  total_bytes: u64,
  completion:  StreamFuture<IOResult>,
  done:        bool,
}

impl<F, R> SourceLogic for ReaderSourceLogic<F, R>
where
  F: FnOnce() -> R + Send + 'static,
  R: Read + Send + 'static,
{
  fn pull(&mut self) -> Result<Option<DynValue>, StreamError> {
    if self.done {
      return Ok(None);
    }

    if self.reader.is_none()
      && let Some(factory) = self.factory.take()
    {
      self.reader = Some(BufReader::new(factory()));
    }

    let Some(reader) = &mut self.reader else {
      self.done = true;
      self.completion.complete(Ok(IOResult::successful(self.total_bytes)));
      return Ok(None);
    };

    let mut buf = vec![0u8; self.chunk_size];
    match reader.read(&mut buf) {
      | Ok(0) => {
        self.done = true;
        self.reader = None;
        self.completion.complete(Ok(IOResult::successful(self.total_bytes)));
        Ok(None)
      },
      | Ok(n) => {
        self.total_bytes += n as u64;
        Ok(Some(Box::new(buf[..n].to_vec())))
      },
      | Err(e) => {
        self.done = true;
        self.reader = None;
        let error = io_error_to_stream_error(&e);
        self.completion.complete(Ok(IOResult::failed(self.total_bytes, error.clone())));
        Err(error)
      },
    }
  }

  fn on_cancel(&mut self) -> Result<(), StreamError> {
    self.done = true;
    self.factory = None;
    self.reader = None;
    self.completion.complete(Ok(IOResult::successful(self.total_bytes)));
    Ok(())
  }
}

impl<F, W> SinkLogic for WriteToWriterSinkLogic<F, W>
where
  F: FnOnce() -> W + Send + 'static,
  W: Write + Send + 'static,
{
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    if let Some(factory) = self.factory.take() {
      self.writer = Some(BufWriter::new(factory()));
    }
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let chunk = *input.downcast::<Vec<u8>>().map_err(|_| StreamError::TypeMismatch)?;
    let Some(writer) = &mut self.writer else {
      // writer が既に破棄されている場合は即完了。
      return Ok(SinkDecision::Complete);
    };
    if let Err(e) = writer.write_all(&chunk) {
      self.writer = None;
      return Err(io_error_to_stream_error(&e));
    }
    self.count += chunk.len() as u64;
    if self.auto_flush
      && let Err(e) = writer.flush()
    {
      self.writer = None;
      return Err(io_error_to_stream_error(&e));
    }
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    let io_result = if let Some(mut writer) = self.writer.take() {
      match writer.flush() {
        | Ok(()) => IOResult::successful(self.count),
        | Err(e) => IOResult::failed(self.count, io_error_to_stream_error(&e)),
      }
    } else {
      IOResult::failed(self.count, StreamError::Failed)
    };
    self.completion.complete(Ok(io_result));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.writer = None;
    self.completion.complete(Ok(IOResult::failed(self.count, error)));
  }
}
