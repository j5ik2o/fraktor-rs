#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unknown_lints)]

//! Standard adaptors for the fraktor stream runtime.

extern crate std;

/// Public std-backed I/O adapter surface.
pub mod io;
/// Public std-backed materializer adapter surface.
pub mod materializer;

use std::io::Error;

use fraktor_stream_core_rs::r#impl::StreamError;

// `std::io::Error` を `StreamError::IoError` に変換する。
fn io_error_to_stream_error(e: &Error) -> StreamError {
  StreamError::IoError { kind: format!("{:?}", e.kind()), message: format!("{e}") }
}
