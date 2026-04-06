use alloc::boxed::Box;
use core::{any::Any, task::Waker};

use super::{
  dispatcher_shared::DispatcherShared, schedule_adapter::ScheduleAdapter,
  schedule_adapter_shared::ScheduleAdapterShared, schedule_waker::ScheduleWaker,
};

/// Inline adapter that delegates to the built-in `ScheduleWaker`.
#[derive(Default)]
pub struct InlineScheduleAdapter;

impl InlineScheduleAdapter {
  /// Creates a new inline adapter.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }

  /// Helper that creates a shared handle with external synchronization.
  #[must_use]
  pub fn shared() -> ScheduleAdapterShared {
    ScheduleAdapterShared::new(Box::new(InlineScheduleAdapter))
  }
}

impl ScheduleAdapter for InlineScheduleAdapter {
  fn create_waker(&mut self, dispatcher: DispatcherShared) -> Waker {
    ScheduleWaker::into_waker(dispatcher)
  }

  fn on_pending(&mut self) {
    core::hint::spin_loop();
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}
