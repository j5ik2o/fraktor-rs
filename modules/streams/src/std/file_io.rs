extern crate std;

#[cfg(test)]
mod tests;

use std::{
  fs,
  io::{BufWriter, Write},
  path::{Path, PathBuf},
};

use crate::core::{
  DemandTracker, DynValue, IOResult, SinkDecision, SinkLogic, StreamCompletion, StreamError,
  stage::{Sink, Source, StageKind},
};

/// File IO utilities for reading and writing byte streams from/to files.
///
/// Corresponds to Pekko's `FileIO` object. Provides `from_path` and `to_path`
/// factory methods that produce sources and sinks with `IOResult` materialized
/// values.
pub struct FileIO;

impl FileIO {
  /// Creates a source that reads all bytes from a file at the given path.
  ///
  /// The file is read eagerly when the source is constructed. Bytes are emitted
  /// as individual `u8` elements. The materialized value is an [`IOResult`]
  /// containing the number of bytes read and the completion status.
  ///
  /// On read failure, the source emits zero elements and the materialized
  /// `IOResult` records the error.
  #[must_use]
  pub fn from_path<P: AsRef<Path>>(path: P) -> Source<u8, IOResult> {
    match fs::read(path.as_ref()) {
      | Ok(bytes) => {
        let count = bytes.len() as u64;
        Source::from_iterator(bytes).map_materialized_value(move |_| IOResult::successful(count))
      },
      | Err(_) => Source::empty().map_materialized_value(|_| IOResult::failed(0, StreamError::Failed)),
    }
  }

  /// Creates a sink that writes received bytes to a file at the given path.
  ///
  /// The file is opened when the stream starts. Each byte is written through a
  /// buffered writer so that intermediate data is flushed to disk incrementally
  /// rather than accumulated entirely in memory. The materialized value is a
  /// [`StreamCompletion<IOResult>`] that completes with the number of bytes
  /// written and the completion status.
  ///
  /// The file is created (or truncated) at the given path. On write failure the
  /// `IOResult` records the error with a byte count of zero.
  #[must_use]
  pub fn to_path<P: AsRef<Path>>(path: P) -> Sink<u8, StreamCompletion<IOResult>> {
    let path_buf = path.as_ref().to_path_buf();
    let completion = StreamCompletion::new();
    let logic =
      WriteToPathSinkLogic { path: path_buf, writer: None, count: 0, completion: completion.clone() };
    Sink::from_definition(StageKind::Custom, logic, completion)
  }
}

struct WriteToPathSinkLogic {
  path:       PathBuf,
  writer:     Option<BufWriter<fs::File>>,
  count:      u64,
  completion: StreamCompletion<IOResult>,
}

impl SinkLogic for WriteToPathSinkLogic {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    if let Ok(file) = fs::File::create(&self.path) {
      self.writer = Some(BufWriter::new(file));
    }
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let byte = *input.downcast::<u8>().map_err(|_| StreamError::TypeMismatch)?;
    if let Some(writer) = &mut self.writer {
      if writer.write_all(&[byte]).is_err() {
        self.writer = None;
      } else {
        self.count += 1;
      }
    }
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    let io_result = if let Some(mut writer) = self.writer.take() {
      match writer.flush() {
        | Ok(()) => IOResult::successful(self.count),
        | Err(_) => IOResult::failed(self.count, StreamError::Failed),
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
