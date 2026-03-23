extern crate std;

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use std::io::{BufReader, BufWriter, Read, Write};

use crate::core::{
  DemandTracker, DynValue, IOResult, SinkDecision, SinkLogic, StreamCompletion, StreamError,
  stage::{Sink, Source, StageKind},
};

/// Adapters for converting between Rust IO types and stream stages.
///
/// Corresponds to Pekko's `StreamConverters` object. Provides `from_reader`
/// and `to_writer` factory methods that bridge `std::io::Read` /
/// `std::io::Write` with stream `Source` / `Sink` stages.
pub struct StreamConverters;

impl StreamConverters {
  /// Creates a source that reads bytes from a reader in chunks.
  ///
  /// The factory closure is called eagerly to obtain the reader. Data is read
  /// in `chunk_size`-byte chunks and each chunk is emitted as a `Vec<u8>`
  /// element. The materialized value is an [`IOResult`] containing the total
  /// number of bytes read and the completion status.
  ///
  /// **Note:** The current implementation reads all data into memory before
  /// emitting chunks. A future version should lazily read on demand, matching
  /// Pekko's `StreamConverters.fromInputStream` behavior.
  ///
  /// On read failure the source emits all chunks read so far and the
  /// `IOResult` records the error.
  #[must_use]
  pub fn from_reader<F>(factory: F, chunk_size: usize) -> Source<alloc::vec::Vec<u8>, IOResult>
  where
    F: FnOnce() -> Box<dyn std::io::Read + Send> + Send + 'static, {
    let raw_reader = factory();
    let mut reader = BufReader::new(raw_reader);
    let mut chunks: alloc::vec::Vec<alloc::vec::Vec<u8>> = alloc::vec::Vec::new();
    let mut total_bytes: u64 = 0;
    let mut buf = alloc::vec![0u8; chunk_size];

    loop {
      match reader.read(&mut buf) {
        | Ok(0) => break,
        | Ok(n) => {
          total_bytes += n as u64;
          chunks.push(buf[..n].to_vec());
        },
        | Err(e) => {
          let error = io_error_to_stream_error(&e);
          return Source::from_iterator(chunks).map_materialized_value(move |_| IOResult::failed(total_bytes, error));
        },
      }
    }

    Source::from_iterator(chunks).map_materialized_value(move |_| IOResult::successful(total_bytes))
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
    if let Some(writer) = &mut self.writer {
      if writer.write_all(&[byte]).is_err() {
        // 書き込み失敗時は writer を破棄。on_complete で IOResult::failed として報告。
        self.writer = None;
      } else {
        self.count += 1;
        if self.auto_flush && writer.flush().is_err() {
          // フラッシュ失敗時も writer を破棄。on_complete で IOResult::failed として報告。
          self.writer = None;
        }
      }
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

use super::io_error_to_stream_error;
