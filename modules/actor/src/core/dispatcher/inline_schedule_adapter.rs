use alloc::boxed::Box;
use core::{any::Any, task::Waker};

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily},
  sync::ArcShared,
};

use super::{
  base::DispatcherGeneric,
  schedule_adapter::{ScheduleAdapter, ScheduleAdapterShared},
  schedule_waker::ScheduleWaker,
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
  pub fn shared<TB: RuntimeToolbox + 'static>() -> ScheduleAdapterShared<TB> {
    ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(Box::new(InlineScheduleAdapter)))
  }
}

impl<TB: RuntimeToolbox + 'static> ScheduleAdapter<TB> for InlineScheduleAdapter {
  fn create_waker(&mut self, dispatcher: DispatcherGeneric<TB>) -> Waker {
    ScheduleWaker::<TB>::into_waker(dispatcher)
  }

  fn on_pending(&mut self) {
    core::hint::spin_loop();
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}
