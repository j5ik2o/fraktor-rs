//! Factory contract for shared tick-driver control handles.

use alloc::boxed::Box;

use fraktor_utils_core_rs::core::sync::SharedLock;

use super::{TickDriverControl, TickDriverControlShared};

/// Materializes a shared tick-driver control handle using the selected lock family.
pub trait TickDriverControlSharedFactory: Send + Sync {
  /// Wraps the control hook into a shared handle.
  fn create_tick_driver_control_shared(&self, control: Box<dyn TickDriverControl>) -> TickDriverControlShared;

  /// Wraps an already materialized shared lock in a shared handle.
  fn create_tick_driver_control_shared_from_shared(
    &self,
    shared: SharedLock<Box<dyn TickDriverControl>>,
  ) -> TickDriverControlShared;
}
