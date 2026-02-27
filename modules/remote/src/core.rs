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
mod endpoint_registry;
/// Endpoint writer for outbound remote messages.
pub mod endpoint_writer;
/// Envelope types for remote message routing.
pub mod envelope;
mod event_publisher;
/// Phi-accrual failure detector for remote nodes.
pub mod failure_detector;
/// Flight recorder for remoting diagnostics.
pub mod flight_recorder;
mod flush;
mod flush_ack;
/// Handshake protocol frames and negotiation.
pub mod handshake;
mod remote_authority_snapshot;
mod remote_instrument;
#[cfg(feature = "tokio-transport")]
mod remote_instruments;
mod remote_node_id;
/// Remoting extension lifecycle and control.
pub mod remoting_extension;
/// Transport layer abstractions and implementations.
pub mod transport;
pub(crate) mod watcher;
mod wire_error;
mod wire_format;

pub use block_list_provider::BlockListProvider;
pub use event_publisher::{EventPublisher, EventPublisherGeneric};
pub use flush::{FLUSH_FRAME_KIND, Flush};
pub use flush_ack::{FLUSH_ACK_FRAME_KIND, FlushAck};
pub use remote_authority_snapshot::RemoteAuthoritySnapshot;
pub use remote_instrument::RemoteInstrument;
#[cfg(feature = "tokio-transport")]
pub(crate) use remote_instruments::RemoteInstruments;
pub use remote_node_id::RemoteNodeId;
pub use wire_error::WireError;

#[cfg(feature = "std")]
pub use crate::std::{RemotingExtensionId, RemotingExtensionInstaller};
