//! Core remoting primitives shared between std and no_std configurations.
#![allow(cfg_std_forbid)]

mod actor_ref_field_normalizer;
mod actor_ref_provider;
mod backpressure;
mod block_list_provider;
mod endpoint_association;
mod endpoint_reader;
mod endpoint_registry;
mod endpoint_writer;
mod envelope;
mod event_publisher;
mod failure_detector;
mod flight_recorder;
mod handshake;
mod remote_authority_snapshot;
mod remote_node_id;
mod remoting_extension;
pub mod transport;
mod watcher;
mod wire_error;

pub use actor_ref_provider::{
  LoopbackActorRefProvider, LoopbackActorRefProviderGeneric, LoopbackActorRefProviderInstaller, RemoteActorRefProvider,
  RemoteActorRefProviderError, RemoteActorRefProviderGeneric, RemoteActorRefProviderInstaller, TokioActorRefProvider,
  TokioActorRefProviderGeneric, TokioActorRefProviderInstaller, default_loopback_setup,
};
pub use backpressure::{FnRemotingBackpressureListener, RemotingBackpressureListener};
pub use block_list_provider::BlockListProvider;
pub use endpoint_association::{
  AssociationState, EndpointAssociationCommand, EndpointAssociationCoordinator, EndpointAssociationCoordinatorShared,
  EndpointAssociationCoordinatorSharedGeneric, EndpointAssociationEffect, EndpointAssociationResult, QuarantineReason,
};
pub use endpoint_reader::{EndpointReader, EndpointReaderError, EndpointReaderGeneric};
pub use endpoint_writer::{
  EndpointWriter, EndpointWriterError, EndpointWriterGeneric, EndpointWriterShared, EndpointWriterSharedGeneric,
};
pub use envelope::{DeferredEnvelope, InboundEnvelope, OutboundMessage, OutboundPriority, RemotingEnvelope};
pub use event_publisher::{EventPublisher, EventPublisherGeneric};
pub use failure_detector::{PhiFailureDetector, PhiFailureDetectorConfig, PhiFailureDetectorEffect};
pub use flight_recorder::{FlightMetricKind, RemotingFlightRecorder, RemotingFlightRecorderSnapshot, RemotingMetric};
pub use handshake::{HandshakeFrame, HandshakeKind};
pub use remote_authority_snapshot::RemoteAuthoritySnapshot;
pub use remote_node_id::RemoteNodeId;
pub use remoting_extension::{
  RemotingControl, RemotingControlHandle, RemotingControlShared, RemotingError, RemotingExtension,
  RemotingExtensionConfig, RemotingExtensionGeneric,
};
pub use transport::{
  InboundFrame, LoopbackTransport, RemoteTransport, RemoteTransportShared, TokioTransportConfig,
  TransportBackpressureHook, TransportBackpressureHookShared, TransportBind, TransportChannel, TransportEndpoint,
  TransportError, TransportFactory, TransportHandle, TransportInbound, TransportInboundShared,
};
pub use wire_error::WireError;

#[cfg(feature = "std")]
pub use crate::std::{RemotingExtensionId, RemotingExtensionInstaller};
