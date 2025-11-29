//! Callback invoked when transports receive inbound frames.

extern crate alloc;

use alloc::boxed::Box;

use fraktor_utils_rs::core::{runtime_toolbox::ToolboxMutex, sync::ArcShared};

use crate::core::transport::transport_inbound_frame::InboundFrame;

/// Receives decoded transport frames and forwards them to higher layers.
///
/// # External Synchronization
///
/// This trait does NOT require `Sync` because it expects callers to provide
/// external synchronization. Typical usage wraps the handler in a mutex
/// provided by the toolbox's `MutexFamily`:
///
/// ```text
/// let handler: TransportInboundShared<TB> = ...;
/// handler.lock().on_frame(frame);
/// ```
///
/// This design allows runtime-specific mutex implementations (e.g., `StdSyncMutex`
/// for std environments or `NoStdMutex` for no_std) to be selected via the
/// `RuntimeToolbox` abstraction.
pub trait TransportInbound: Send + 'static {
  /// Handles a single inbound frame.
  fn on_frame(&mut self, frame: InboundFrame);
}

/// Shared handle to a [`TransportInbound`] implementation with external synchronization.
///
/// The mutex type is determined by the `RuntimeToolbox`'s `MutexFamily`, allowing
/// the same code to use `StdSyncMutex` in std environments or `NoStdMutex` in no_std.
pub type TransportInboundShared<TB> = ArcShared<ToolboxMutex<Box<dyn TransportInbound + 'static>, TB>>;
