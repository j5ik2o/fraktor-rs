extern crate std;

use std::{
  boxed::Box,
  io::{BufReader, BufWriter, Read, Write},
  vec::Vec,
};

use fraktor_stream_core_rs::core::{
  DemandTracker, DynValue, IOResult, SinkDecision, SinkLogic, SourceLogic, StreamError,
  dsl::{Sink, Source},
  materialization::StreamCompletion,
  stage::StageKind,
};

use super::super::io_error_to_stream_error;

/// Adapters for converting between Rust IO types and stream stages.
///
/// Corresponds to Pekko's `StreamConverters` object. Provides `from_reader`
/// and `to_writer` factory methods that bridge `std::io::Read` /
/// `std::io::Write` with stream `Source` / `Sink` stages.
pub struct StreamConverters;

impl StreamConverters {
  /// Creates a source that reads bytes from a reader in chunks.
  ///
  /// The factory closure is called lazily on the first pull to obtain the
  /// reader. Data is read lazily on each pull in `chunk_size`-byte chunks and
  /// each chunk is emitted as a `Vec<u8>` element.
  ///
  /// The materialized value is a [`StreamCompletion<IOResult>`] that completes
  /// with the total number of bytes read and the completion status.
  #[must_use]
  pub fn from_reader<F>(factory: F, chunk_size: usize) -> Source<Vec<u8>, StreamCompletion<IOResult>>
  where
    F: FnOnce() -> Box<dyn std::io::Read + Send> + Send + 'static, {
    let completion = StreamCompletion::new();
    if chunk_size == 0 {
      let error = std::io::Error::new(std::io::ErrorKind::InvalidInput, "chunk_size must be greater than 0");
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

  /// Creates a sink that writes received bytes to a writer.
  ///
  /// The factory closure is called when the stream starts to obtain the
  /// writer. Each received `u8` element is written through a buffered writer.
  /// When `auto_flush` is `true`, the writer is flushed after every element.
  /// The materialized value is a [`StreamCompletion<IOResult>`] that completes
  /// with the total number of bytes written and the completion status.
  #[must_use]
  pub fn to_writer<F>(factory: F, auto_flush: bool) -> Sink<u8, StreamCompletion<IOResult>>
  where
    F: FnOnce() -> Box<dyn std::io::Write + Send> + Send + 'static, {
    let completion = StreamCompletion::new();
    let logic = WriteToWriterSinkLogic {
      factory: Some(factory),
      auto_flush,
      writer: None,
      count: 0,
      completion: completion.clone(),
    };
    Sink::from_definition(StageKind::Custom, logic, completion)
  }
}

struct WriteToWriterSinkLogic<F> {
  factory:    Option<F>,
  auto_flush: bool,
  writer:     Option<BufWriter<Box<dyn std::io::Write + Send>>>,
  count:      u64,
  completion: StreamCompletion<IOResult>,
}

struct ReaderSourceLogic<F> {
  factory:     Option<F>,
  reader:      Option<BufReader<Box<dyn std::io::Read + Send>>>,
  chunk_size:  usize,
  total_bytes: u64,
  completion:  StreamCompletion<IOResult>,
  done:        bool,
}

impl<F> SourceLogic for ReaderSourceLogic<F>
where
  F: FnOnce() -> Box<dyn std::io::Read + Send> + Send + 'static,
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

impl<F> SinkLogic for WriteToWriterSinkLogic<F>
where
  F: FnOnce() -> Box<dyn std::io::Write + Send> + Send + 'static,
{
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    if let Some(factory) = self.factory.take() {
      self.writer = Some(BufWriter::new(factory()));
    }
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let byte = *input.downcast::<u8>().map_err(|_| StreamError::TypeMismatch)?;
    let Some(writer) = &mut self.writer else {
      // writer が既に破棄されている場合は即完了。
      return Ok(SinkDecision::Complete);
    };
    if let Err(e) = writer.write_all(&[byte]) {
      self.writer = None;
      return Err(io_error_to_stream_error(&e));
    }
    self.count += 1;
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
