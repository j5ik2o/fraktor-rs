//! Std-backed I/O adapter surface.

mod file_io;
mod source;
mod stream_converters;

pub use file_io::FileIO;
pub use stream_converters::StreamConverters;
