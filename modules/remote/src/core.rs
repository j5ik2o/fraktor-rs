//! Core remoting primitives shared between std and no_std configurations.
#![allow(cfg_std_forbid)]

mod actor_ref_field_normalizer;
/// Actor reference provider implementations for remote messaging.
pub mod actor_ref_provider;
/// Backpressure listeners for remoting channels.
pub mod backpressure;
mod block_list_provider;
/// Endpoint association state machine and coordination.
pub mod endpoint_association;
/// Endpoint reader for inbound remote messages.
pub mod endpoint_reader;
/// Endpoint writer for outbound remote messages.
pub mod endpoint_writer;
/// Envelope types for remote message routing.
pub mod envelope;
mod event_publisher;
/// Phi-accrual failure detector for remote nodes.
pub mod failure_detector;
/// Handshake protocol frames and negotiation.
pub mod handshake;
/// Instrumentation and observability for remoting.
pub mod instrument;
mod remote_authority_snapshot;
mod remote_node_id;
/// Remoting extension lifecycle and control.
pub mod remoting_extension;
/// Transport layer abstractions and implementations.
pub mod transport;
pub(crate) mod watcher;
/// Wire protocol primitives: binary framing, encoding errors, and control frames.
pub mod wire;

pub use block_list_provider::BlockListProvider;
pub use event_publisher::EventPublisher;
pub use remote_authority_snapshot::RemoteAuthoritySnapshot;
pub use remote_node_id::RemoteNodeId;

#[cfg(feature = "std")]
pub use crate::std::{RemotingExtensionId, RemotingExtensionInstaller};
