//! Tick driver trait and identifier helpers.

use alloc::boxed::Box;
use core::sync::atomic::Ordering;

use portable_atomic::AtomicU64;

use super::{TickDriverError, TickDriverId, TickDriverKind, TickDriverProvision, TickFeedHandle};
use crate::actor::scheduler::tick_driver::SchedulerTickExecutor;

static NEXT_DRIVER_ID: AtomicU64 = AtomicU64::new(1);

/// Allocates a unique identifier for tick drivers.
#[must_use]
pub fn next_tick_driver_id() -> TickDriverId {
  TickDriverId::new(NEXT_DRIVER_ID.fetch_add(1, Ordering::Relaxed))
}

/// Common contract implemented by environment-specific tick drivers.
///
/// The driver consumes itself (`Box<Self>`) during provisioning so that
/// ownership of all resources is transferred into [`TickDriverProvision`].
pub trait TickDriver: Send + 'static {
  /// Kind classification for observability purposes.
  fn kind(&self) -> TickDriverKind;

  /// Provisions the driver and returns its running state.
  ///
  /// The driver receives a [`TickFeedHandle`] for injecting ticks and a
  /// [`SchedulerTickExecutor`] for driving the scheduler executor from a
  /// background thread or task.
  ///
  /// # Errors
  ///
  /// Returns [`TickDriverError`] when provisioning fails.
  fn provision(
    self: Box<Self>,
    feed: TickFeedHandle,
    executor: SchedulerTickExecutor,
  ) -> Result<TickDriverProvision, TickDriverError>;
}
