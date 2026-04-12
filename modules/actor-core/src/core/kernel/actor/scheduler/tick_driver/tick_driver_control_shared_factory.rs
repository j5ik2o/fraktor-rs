//! Factory contract for shared tick-driver control handles.

use alloc::boxed::Box;

use super::{TickDriverControl, TickDriverControlShared};

/// Materializes a shared tick-driver control handle using the selected lock family.
pub trait TickDriverControlSharedFactory: Send + Sync {
  /// Wraps the control hook into a shared handle.
  fn create_tick_driver_control_shared(&self, control: Box<dyn TickDriverControl>) -> TickDriverControlShared;
}
