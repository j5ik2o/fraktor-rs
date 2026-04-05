extern crate std;

/// Public std-backed I/O adapter surface.
pub mod io;
/// Public std-backed materializer adapter surface.
pub mod materializer;

use fraktor_stream_rs::core::r#impl::StreamError;

// `std::io::Error` を `StreamError::IoError` に変換する。
fn io_error_to_stream_error(e: &std::io::Error) -> StreamError {
  StreamError::IoError { kind: alloc::format!("{:?}", e.kind()), message: alloc::format!("{e}") }
}
