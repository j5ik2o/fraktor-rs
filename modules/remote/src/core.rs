//! Core remoting primitives shared between std and no_std configurations.

pub mod association_state;
pub mod deferred_envelope;
pub mod endpoint_manager;
pub mod endpoint_registry;
pub mod quarantine_reason;
pub mod remote_authority_snapshot;
pub mod remote_node_id;
pub mod remoting_backpressure_listener;
pub mod remoting_control;
pub mod remoting_control_handle;
pub mod remoting_error;
pub mod remoting_extension;
pub mod remoting_extension_config;
pub mod remoting_extension_id;
pub mod transport;

pub use association_state::AssociationState;
pub use deferred_envelope::DeferredEnvelope;
pub use endpoint_manager::{EndpointManager, EndpointManagerCommand, EndpointManagerEffect, EndpointManagerResult};
pub use quarantine_reason::QuarantineReason;
pub use remote_authority_snapshot::RemoteAuthoritySnapshot;
pub use remote_node_id::RemoteNodeId;
pub use remoting_backpressure_listener::{FnRemotingBackpressureListener, RemotingBackpressureListener};
pub use remoting_control::RemotingControl;
pub use remoting_control_handle::RemotingControlHandle;
pub use remoting_error::RemotingError;
pub use remoting_extension::RemotingExtension;
pub use remoting_extension_config::RemotingExtensionConfig;
pub use remoting_extension_id::RemotingExtensionId;
pub use transport::{
  LoopbackTransport, RemoteTransport, TransportBackpressureHook, TransportBind, TransportChannel, TransportEndpoint,
  TransportError, TransportFactory, TransportHandle,
};
