//! Tick driver trait and identifier helpers.

use core::{sync::atomic::Ordering, time::Duration};

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;
use portable_atomic::AtomicU64;

use super::{TickDriverError, TickDriverHandleGeneric, TickDriverId, TickDriverKind, TickFeedHandle};

static NEXT_DRIVER_ID: AtomicU64 = AtomicU64::new(1);

/// Allocates a unique identifier for tick drivers.
#[must_use]
pub fn next_tick_driver_id() -> TickDriverId {
  TickDriverId::new(NEXT_DRIVER_ID.fetch_add(1, Ordering::Relaxed))
}

/// Common contract implemented by environment-specific tick drivers.
///
/// # Interior Mutability Removed
///
/// This trait no longer assumes interior mutability. The `start` method now
/// requires `&mut self`. If shared access is needed, wrap implementations in
/// an external synchronization primitive (e.g., `Mutex<Box<dyn TickDriver>>`).
pub trait TickDriver<TB: RuntimeToolbox>: Send + 'static {
  /// Unique identifier assigned to the driver instance.
  fn id(&self) -> TickDriverId;
  /// Kind classification for observability purposes.
  fn kind(&self) -> TickDriverKind;
  /// Tick resolution produced by this driver.
  fn resolution(&self) -> Duration;
  /// Starts the driver and returns a handle that can be used to stop it later.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError`] when the driver fails to initialize.
  fn start(&mut self, feed: TickFeedHandle<TB>) -> Result<TickDriverHandleGeneric<TB>, TickDriverError>;
}
