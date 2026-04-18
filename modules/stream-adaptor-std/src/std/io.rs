//! Std-backed I/O adapter surface.

mod file_io;
mod source_factory;
mod stream_converters;
mod stream_input_stream;
mod stream_output_stream;

pub use file_io::FileIO;
pub use source_factory::SourceFactory;
pub use stream_converters::StreamConverters;
pub use stream_input_stream::StreamInputStream;
pub use stream_output_stream::StreamOutputStream;
