use core::task::Waker;

use fraktor_utils_core_rs::core::runtime_toolbox::RuntimeToolbox;

use super::{base::DispatcherGeneric, schedule_adapter::ScheduleAdapter, schedule_waker::ScheduleWaker};

/// Inline adapter that delegates to the built-in `ScheduleWaker`.
#[derive(Default)]
pub struct InlineScheduleAdapter;

impl InlineScheduleAdapter {
  /// Creates a new inline adapter.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl<TB: RuntimeToolbox + 'static> ScheduleAdapter<TB> for InlineScheduleAdapter {
  fn create_waker(&self, dispatcher: DispatcherGeneric<TB>) -> Waker {
    ScheduleWaker::<TB>::into_waker(dispatcher)
  }

  fn on_pending(&self) {
    core::hint::spin_loop();
  }
}
