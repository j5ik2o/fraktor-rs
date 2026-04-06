//! TCP-based implementation of `fraktor_remote_core_rs::transport::RemoteTransport`.
//!
//! The module is structured as a thin I/O layer over the pure `remote-core` types:
//!
//! - [`wire_frame::WireFrame`] is the unified on-the-wire enum that multiplexes all five PDU kinds
//!   over a single `tokio_util::codec::Framed` stream.
//! - [`frame_codec::WireFrameCodec`] implements `tokio_util::codec::{Encoder, Decoder}` and
//!   delegates the actual encoding to the core `Codec<T>` implementations.
//! - [`server::TcpServer`] owns a `tokio::net::TcpListener` + the accept loop task.
//! - [`client::TcpClient`] owns a single outbound connection with reader/writer tasks.
//! - [`base::TcpRemoteTransport`] aggregates the above and implements the core port.

#[cfg(test)]
mod tests;

mod base;
mod client;
mod frame_codec;
mod frame_codec_error;
mod inbound_frame_event;
mod server;
mod wire_frame;

pub use base::TcpRemoteTransport;
pub use client::TcpClient;
pub use frame_codec::WireFrameCodec;
pub use frame_codec_error::FrameCodecError;
pub use inbound_frame_event::InboundFrameEvent;
pub use server::TcpServer;
pub use wire_frame::WireFrame;
