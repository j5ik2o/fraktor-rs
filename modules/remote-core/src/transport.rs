//! Transport port: the single trait through which the remote subsystem talks to
//! any concrete byte-stream implementation.
//!
//! This module defines only the contract (trait + data types + error enum).
//! Concrete implementations such as `TcpRemoteTransport` live in the
//! `fraktor-remote-adaptor-std-rs` crate.

mod backpressure_signal;
mod remote_transport;
mod transport_bind;
mod transport_endpoint;
mod transport_error;

pub use backpressure_signal::BackpressureSignal;
pub use remote_transport::RemoteTransport;
pub use transport_bind::TransportBind;
pub use transport_endpoint::TransportEndpoint;
pub use transport_error::TransportError;
