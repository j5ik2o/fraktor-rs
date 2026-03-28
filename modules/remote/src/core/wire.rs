//! Wire protocol primitives: binary framing, encoding errors, and control frames.

mod flush;
mod flush_ack;
mod wire_error;
mod wire_format;

pub use flush::{FLUSH_FRAME_KIND, Flush};
pub use flush_ack::{FLUSH_ACK_FRAME_KIND, FlushAck};
pub use wire_error::WireError;
pub(crate) use wire_format::{read_bool, read_string, write_bool, write_string};
