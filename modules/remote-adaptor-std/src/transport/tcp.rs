//! TCP-based implementation of `fraktor_remote_core_rs::transport::RemoteTransport`.
//!
//! The public surface is intentionally limited to [`TcpRemoteTransport`].
//! Frame codecs, listener tasks, and outbound clients are adapter runtime
//! internals over the pure `remote-core` types.

#[cfg(test)]
#[path = "tcp_test.rs"]
mod tests;

mod base;
mod client;
mod compression;
mod connection_loss_reporter;
mod frame_codec;
mod frame_codec_error;
mod inbound_frame_event;
mod server;
mod wire_frame;

pub use base::TcpRemoteTransport;
pub(crate) use inbound_frame_event::InboundFrameEvent;
pub(crate) use wire_frame::WireFrame;
