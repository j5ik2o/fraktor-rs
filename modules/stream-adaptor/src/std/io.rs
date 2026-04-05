//! Std-backed I/O adapter surface.

mod file_io;
mod source_factory;
mod stream_converters;

pub use file_io::FileIO;
pub use source_factory::SourceFactory;
pub use stream_converters::StreamConverters;
