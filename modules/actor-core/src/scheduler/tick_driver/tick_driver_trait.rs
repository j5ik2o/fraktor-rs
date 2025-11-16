//! Tick driver trait and identifier helpers.

use core::{sync::atomic::Ordering, time::Duration};

use portable_atomic::AtomicU64;

use super::{TickDriverError, TickDriverHandle, TickDriverId, TickDriverKind, TickFeedHandle};
use crate::RuntimeToolbox;

static NEXT_DRIVER_ID: AtomicU64 = AtomicU64::new(1);

/// Allocates a unique identifier for tick drivers.
#[must_use]
pub fn next_tick_driver_id() -> TickDriverId {
  TickDriverId::new(NEXT_DRIVER_ID.fetch_add(1, Ordering::Relaxed))
}

/// Common contract implemented by environment-specific tick drivers.
pub trait TickDriver<TB: RuntimeToolbox>: Send + Sync + 'static {
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
  fn start(&self, feed: TickFeedHandle<TB>) -> Result<TickDriverHandle, TickDriverError>;
}
