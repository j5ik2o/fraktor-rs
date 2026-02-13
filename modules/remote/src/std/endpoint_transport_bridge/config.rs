use alloc::string::String;
use core::time::Duration;

use fraktor_actor_rs::core::system::ActorSystemWeakGeneric;
use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  EventPublisherGeneric, endpoint_reader::EndpointReaderGeneric, endpoint_writer::EndpointWriterSharedGeneric,
  transport::RemoteTransportShared,
};

/// Configuration required to bootstrap the transport bridge.
pub struct EndpointTransportBridgeConfig<TB: RuntimeToolbox + 'static> {
  /// Actor system providing scheduling and state access (weak reference).
  pub system:            ActorSystemWeakGeneric<TB>,
  /// Shared endpoint writer feeding outbound frames.
  pub writer:            EndpointWriterSharedGeneric<TB>,
  /// Shared endpoint reader decoding inbound frames.
  pub reader:            ArcShared<EndpointReaderGeneric<TB>>,
  /// Active transport implementation wrapped in a mutex for shared mutable access.
  pub transport:         RemoteTransportShared<TB>,
  /// Event publisher for lifecycle/backpressure events.
  pub event_publisher:   EventPublisherGeneric<TB>,
  /// Canonical host used when binding listeners.
  pub canonical_host:    String,
  /// Canonical port used when binding listeners.
  pub canonical_port:    u16,
  /// Logical system name advertised during handshakes.
  pub system_name:       String,
  /// Timeout used while waiting for a handshake to complete.
  pub handshake_timeout: Duration,
}
