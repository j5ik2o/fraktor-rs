//! Helper utilities for constructing dispatcher-driven wakers.

use core::{
  marker::PhantomData,
  task::{RawWaker, RawWakerVTable, Waker},
};

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use super::base::DispatcherGeneric;
use crate::core::mailbox::ScheduleHints;

#[cfg(test)]
mod tests;

struct ScheduleShared<TB: RuntimeToolbox + 'static> {
  dispatcher: DispatcherGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> ScheduleShared<TB> {
  const fn new(dispatcher: DispatcherGeneric<TB>) -> Self {
    Self { dispatcher }
  }

  fn schedule(&self) {
    self.dispatcher.register_for_execution(ScheduleHints {
      has_system_messages: false,
      has_user_messages:   true,
      backpressure_active: false,
    });
  }
}

/// Helper for creating a [`Waker`] that reschedules the dispatcher.
pub(crate) struct ScheduleWaker<TB: RuntimeToolbox + 'static> {
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> ScheduleWaker<TB> {
  /// Creates a waker that schedules the dispatcher using the provided dispatcher handle.
  pub(crate) fn into_waker(dispatcher: DispatcherGeneric<TB>) -> Waker {
    let handle = ArcShared::new(ScheduleShared::new(dispatcher));
    unsafe { Waker::from_raw(Self::raw_waker(handle)) }
  }

  unsafe fn raw_waker(handle: ArcShared<ScheduleShared<TB>>) -> RawWaker {
    let data = ArcShared::into_raw(handle) as *const ();
    RawWaker::new(data, &ScheduleWakerVtable::<TB>::VTABLE)
  }

  unsafe fn clone(ptr: *const ()) -> RawWaker {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared<TB>) };
    let clone = handle.clone();
    let _ = ArcShared::into_raw(handle);
    unsafe { Self::raw_waker(clone) }
  }

  unsafe fn wake(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared<TB>) };
    handle.schedule();
  }

  unsafe fn wake_by_ref(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared<TB>) };
    handle.schedule();
    let _ = ArcShared::into_raw(handle);
  }

  unsafe fn drop(ptr: *const ()) {
    let _ = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared<TB>) };
  }
}

struct ScheduleWakerVtable<TB: RuntimeToolbox + 'static>(PhantomData<TB>);

impl<TB: RuntimeToolbox + 'static> ScheduleWakerVtable<TB> {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(
    ScheduleWaker::<TB>::clone,
    ScheduleWaker::<TB>::wake,
    ScheduleWaker::<TB>::wake_by_ref,
    ScheduleWaker::<TB>::drop,
  );
}
