//! Callback invoked when transports receive inbound frames.

use crate::core::transport::transport_inbound_frame::InboundFrame;

/// Receives decoded transport frames and forwards them to higher layers.
pub trait TransportInbound: Send + Sync + 'static {
  /// Handles a single inbound frame.
  fn on_frame(&self, frame: InboundFrame);
}
