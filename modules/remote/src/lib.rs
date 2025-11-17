#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(test), no_std)]

//! Remoting facilities for the fraktor actor runtime.

extern crate alloc;

mod backpressure_listener;
mod endpoint_supervisor;
mod endpoint_manager;
mod remoting_connection_snapshot;
mod remoting_control;
mod remoting_control_handle;
mod remoting_error;
mod remoting_extension;
mod remoting_extension_config;
mod remoting_extension_id;
mod transport;

pub use backpressure_listener::RemotingBackpressureListener;
pub use fraktor_actor_rs::core::event_stream::BackpressureSignal;
pub use remoting_connection_snapshot::RemotingConnectionSnapshot;
pub use remoting_control::RemotingControl;
pub use remoting_control_handle::RemotingControlHandle;
pub use remoting_error::RemotingError;
pub use remoting_extension::RemotingExtension;
pub use remoting_extension_config::RemotingExtensionConfig;
pub use remoting_extension_id::RemotingExtensionId;
pub use transport::{
  factory::TransportFactory,
  LoopbackTransport,
  RemoteTransport,
  TransportBind,
  TransportEndpoint,
  TransportChannel,
  TransportHandle,
  TransportError,
};
pub use endpoint_manager::{AssociationState, EndpointManager, EndpointManagerCommand, RemoteNodeId};
