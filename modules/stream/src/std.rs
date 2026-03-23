/// File IO utilities for reading and writing byte streams.
mod file_io;
/// Std-backed source adapters.
mod source;
/// Adapters for converting between Rust IO types and stream stages.
mod stream_converters;

pub use file_io::FileIO;
pub use stream_converters::StreamConverters;

use crate::core::StreamError;

// `std::io::Error` を `StreamError::IoError` に変換する。
fn io_error_to_stream_error(e: &std::io::Error) -> StreamError {
  StreamError::IoError { kind: alloc::format!("{:?}", e.kind()), message: alloc::format!("{e}") }
}
