extern crate std;

/// File IO utilities for reading and writing byte streams.
mod file_io;
/// Public std-backed I/O adapter surface.
pub mod io;
/// Public std-backed materializer adapter surface.
pub mod materializer;
/// Std-backed source adapters.
mod source;
/// Adapters for converting between Rust IO types and stream stages.
mod stream_converters;
/// Per-ActorSystem shared materializer extension.
mod system_materializer;
/// Extension ID for SystemMaterializer.
mod system_materializer_id;

use crate::core::StreamError;

// `std::io::Error` を `StreamError::IoError` に変換する。
fn io_error_to_stream_error(e: &std::io::Error) -> StreamError {
  StreamError::IoError { kind: alloc::format!("{:?}", e.kind()), message: alloc::format!("{e}") }
}
