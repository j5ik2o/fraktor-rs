extern crate std;

#[cfg(test)]
mod tests;

use std::{
  fs,
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

  /// Creates a sink that writes all received bytes to a file at the given path.
  ///
  /// Bytes are accumulated internally and written to disk when the stream
  /// completes. The materialized value is a [`StreamCompletion<IOResult>`]
  /// that completes with the number of bytes written and the completion status.
  ///
  /// The file is created (or truncated) at the given path. On write failure the
  /// `IOResult` records the error with a byte count of zero.
  #[must_use]
  pub fn to_path<P: AsRef<Path>>(path: P) -> Sink<u8, StreamCompletion<IOResult>> {
    let path_buf = path.as_ref().to_path_buf();
    let completion = StreamCompletion::new();
    let logic =
      WriteToPathSinkLogic { path: path_buf, buffer: alloc::vec::Vec::new(), completion: completion.clone() };
    Sink::from_definition(StageKind::Custom, logic, completion)
  }
}

struct WriteToPathSinkLogic {
  path:       PathBuf,
  buffer:     alloc::vec::Vec<u8>,
  completion: StreamCompletion<IOResult>,
}

impl SinkLogic for WriteToPathSinkLogic {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    demand.request(1)
  }

  fn on_push(&mut self, input: DynValue, demand: &mut DemandTracker) -> Result<SinkDecision, StreamError> {
    let byte = *input.downcast::<u8>().map_err(|_| StreamError::TypeMismatch)?;
    self.buffer.push(byte);
    demand.request(1)?;
    Ok(SinkDecision::Continue)
  }

  fn on_complete(&mut self) -> Result<(), StreamError> {
    let count = self.buffer.len() as u64;
    let io_result = match fs::write(&self.path, &self.buffer) {
      | Ok(()) => IOResult::successful(count),
      | Err(_) => IOResult::failed(0, StreamError::Failed),
    };
    self.completion.complete(Ok(io_result));
    Ok(())
  }

  fn on_error(&mut self, error: StreamError) {
    self.completion.complete(Ok(IOResult::failed(0, error)));
  }
}
