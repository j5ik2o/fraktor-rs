use alloc::{string::String, sync::Arc, vec::Vec};
use core::time::Duration;

use fraktor_actor_rs::core::kernel::system::ActorSystemWeak;
use fraktor_utils_rs::core::sync::ArcShared;

use crate::core::{
  EventPublisher, endpoint_reader::EndpointReader, endpoint_writer::EndpointWriterShared, instrument::RemoteInstrument,
  remoting_extension::RemotingControlHandle, transport::RemoteTransportShared,
};

/// Configuration required to bootstrap the transport bridge.
pub struct EndpointTransportBridgeConfig {
  /// Actor system providing scheduling and state access (weak reference).
  pub system:                 ActorSystemWeak,
  /// Remoting control handle used to dispatch watcher commands.
  pub control:                RemotingControlHandle,
  /// Shared endpoint writer feeding outbound frames.
  pub writer:                 EndpointWriterShared,
  /// Shared endpoint reader decoding inbound frames.
  pub reader:                 ArcShared<EndpointReader>,
  /// Active transport implementation wrapped in a mutex for shared mutable access.
  pub transport:              RemoteTransportShared,
  /// Event publisher for lifecycle/backpressure events.
  pub event_publisher:        EventPublisher,
  /// Canonical host used when binding listeners.
  pub canonical_host:         String,
  /// Canonical port used when binding listeners.
  pub canonical_port:         u16,
  /// Logical system name advertised during handshakes.
  pub system_name:            String,
  /// Registered remoting instruments used by outbound/inbound pipelines.
  pub remote_instruments:     Vec<Arc<dyn RemoteInstrument>>,
  /// Timeout used while waiting for a handshake to complete.
  pub handshake_timeout:      Duration,
  /// Timeout applied when flushing outstanding messages during graceful shutdown.
  pub shutdown_flush_timeout: Duration,
  /// Maximum number of outbound system messages kept pending per authority.
  pub ack_send_window:        u64,
  /// Maximum accepted sequence distance for inbound acked-delivery per authority.
  pub ack_receive_window:     u64,
}
