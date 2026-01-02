use alloc::boxed::Box;
use core::{any::Any, task::Waker};

use fraktor_utils_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{
  dispatcher_shared::DispatcherSharedGeneric, schedule_adapter::ScheduleAdapter,
  schedule_adapter_shared::ScheduleAdapterSharedGeneric, schedule_waker::ScheduleWaker,
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
  pub fn shared<TB: RuntimeToolbox + 'static>() -> ScheduleAdapterSharedGeneric<TB> {
    ScheduleAdapterSharedGeneric::new(Box::new(InlineScheduleAdapter))
  }
}

impl<TB: RuntimeToolbox + 'static> ScheduleAdapter<TB> for InlineScheduleAdapter {
  fn create_waker(&mut self, dispatcher: DispatcherSharedGeneric<TB>) -> Waker {
    ScheduleWaker::<TB>::into_waker(dispatcher)
  }

  fn on_pending(&mut self) {
    core::hint::spin_loop();
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}
