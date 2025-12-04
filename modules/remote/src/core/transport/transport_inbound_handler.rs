//! Callback invoked when transports receive inbound frames.

use crate::core::transport::transport_inbound_frame::InboundFrame;

/// Receives decoded transport frames and forwards them to higher layers.
///
/// # External Synchronization
///
/// This trait does NOT require `Sync` because it expects callers to provide
/// external synchronization. Typical usage wraps the handler in
/// [`TransportInboundShared`](super::TransportInboundShared) and uses
/// `with_write` to access the handler:
///
/// ```text
/// let handler: TransportInboundShared<TB> = TransportInboundShared::new(boxed_handler);
/// handler.with_write(|h| h.on_frame(frame));
/// ```
///
/// This design allows runtime-specific mutex implementations (e.g., `StdSyncMutex`
/// for std environments or `NoStdMutex` for no_std) to be selected via the
/// `RuntimeToolbox` abstraction.
pub trait TransportInbound: Send + 'static {
  /// Handles a single inbound frame.
  fn on_frame(&mut self, frame: InboundFrame);
}
