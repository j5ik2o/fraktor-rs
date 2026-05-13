extern crate std;

#[cfg(test)]
#[path = "file_io_test.rs"]
mod tests;

use std::{
  fs::{self, File, OpenOptions},
  io::{BufWriter, Error, ErrorKind, Read, Seek, SeekFrom, Write},
  path::{Path, PathBuf},
};

use fraktor_stream_core_kernel_rs::{
  DemandTracker, DynValue, IOResult, SinkDecision, SinkLogic, StreamError,
  dsl::{Sink, Source},
  materialization::StreamFuture,
  stage::StageKind,
};

use super::super::io_error_to_stream_error;

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
      | Err(e) => Source::empty().map_materialized_value(move |_| IOResult::failed(0, io_error_to_stream_error(&e))),
    }
  }

  /// Creates a source that reads bytes from a file with configurable options.
  ///
  /// Reads all bytes starting from `start_position`, using `chunk_size` as the
  /// internal read buffer size. If the start position is past the end of the
  /// file, the source emits zero elements.
  ///
  /// Corresponds to Pekko's `FileIO.fromPath(f, chunkSize, startPosition)`.
  #[must_use]
  pub fn from_path_with_options<P: AsRef<Path>>(
    path: P,
    chunk_size: usize,
    start_position: u64,
  ) -> Source<u8, IOResult> {
    if chunk_size == 0 {
      let error = Error::new(ErrorKind::InvalidInput, "chunk_size must be greater than 0");
      return Source::empty().map_materialized_value(move |_| IOResult::failed(0, io_error_to_stream_error(&error)));
    }

    let result = (|| -> Result<Vec<u8>, Error> {
      let mut file = File::open(path.as_ref())?;
      file.seek(SeekFrom::Start(start_position))?;
      let mut all_bytes = Vec::new();
      let mut buf = vec![0u8; chunk_size];
      loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
          break;
        }
        all_bytes.extend_from_slice(&buf[..n]);
      }
      Ok(all_bytes)
    })();

    match result {
      | Ok(bytes) => {
        let count = bytes.len() as u64;
        Source::from_iterator(bytes).map_materialized_value(move |_| IOResult::successful(count))
      },
      | Err(e) => Source::empty().map_materialized_value(move |_| IOResult::failed(0, io_error_to_stream_error(&e))),
    }
  }

  /// Creates a sink that writes received bytes to a file at the given path.
  ///
  /// The file is opened when the stream starts. Each byte is written through a
  /// buffered writer so that intermediate data is flushed to disk incrementally
  /// rather than accumulated entirely in memory. The materialized value is a
  /// [`StreamFuture<IOResult>`] that completes with the number of bytes
  /// written and the completion status.
  ///
  /// The file is created (or truncated) at the given path. On write failure the
  /// `IOResult` records the error with a byte count of zero.
  #[must_use]
  pub fn to_path<P: AsRef<Path>>(path: P) -> Sink<u8, StreamFuture<IOResult>> {
    let path_buf = path.as_ref().to_path_buf();
    let completion = StreamFuture::new();
    let logic = WriteToPathSinkLogic {
      path:           path_buf,
      options:        None,
      start_position: None,
      writer:         None,
      count:          0,
      completion:     completion.clone(),
    };
    Sink::from_definition(StageKind::Custom, logic, completion)
  }

  /// Creates a sink that writes received bytes to a file with the given open
  /// options.
  ///
  /// The `options` parameter controls how the file is opened (e.g. append,
  /// create, truncate). Corresponds to Pekko's `FileIO.toPath(f, options)`.
  #[must_use]
  pub fn to_path_with_options<P: AsRef<Path>>(path: P, options: OpenOptions) -> Sink<u8, StreamFuture<IOResult>> {
    let path_buf = path.as_ref().to_path_buf();
    let completion = StreamFuture::new();
    let logic = WriteToPathSinkLogic {
      path:           path_buf,
      options:        Some(options),
      start_position: None,
      writer:         None,
      count:          0,
      completion:     completion.clone(),
    };
    Sink::from_definition(StageKind::Custom, logic, completion)
  }

  /// Creates a sink that writes received bytes to a file at the given position.
  ///
  /// The `start_position` parameter specifies the byte offset at which writing
  /// begins. Corresponds to Pekko's `FileIO.toPath(f, options, startPosition)`.
  #[must_use]
  pub fn to_path_with_position<P: AsRef<Path>>(
    path: P,
    options: OpenOptions,
    start_position: u64,
  ) -> Sink<u8, StreamFuture<IOResult>> {
    let path_buf = path.as_ref().to_path_buf();
    let completion = StreamFuture::new();
    let logic = WriteToPathSinkLogic {
      path:           path_buf,
      options:        Some(options),
      start_position: Some(start_position),
      writer:         None,
      count:          0,
      completion:     completion.clone(),
    };
    Sink::from_definition(StageKind::Custom, logic, completion)
  }
}

struct WriteToPathSinkLogic {
  path:           PathBuf,
  options:        Option<OpenOptions>,
  start_position: Option<u64>,
  writer:         Option<BufWriter<File>>,
  count:          u64,
  completion:     StreamFuture<IOResult>,
}

impl SinkLogic for WriteToPathSinkLogic {
  fn on_start(&mut self, demand: &mut DemandTracker) -> Result<(), StreamError> {
    let mut file = if let Some(options) = self.options.take() {
      options.open(&self.path).map_err(|e| io_error_to_stream_error(&e))?
    } else {
      File::create(&self.path).map_err(|e| io_error_to_stream_error(&e))?
    };
    if let Some(pos) = self.start_position {
      file.seek(SeekFrom::Start(pos)).map_err(|e| io_error_to_stream_error(&e))?;
    }
    self.writer = Some(BufWriter::new(file));
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
