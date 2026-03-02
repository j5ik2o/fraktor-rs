//! Helper utilities for constructing dispatcher-driven wakers.

use core::{
  marker::PhantomData,
  task::{RawWaker, RawWakerVTable, Waker},
};

use fraktor_utils_rs::core::{runtime_toolbox::RuntimeMutex, sync::ArcShared};

use super::dispatcher_shared::DispatcherShared;
use crate::core::dispatch::mailbox::ScheduleHints;

#[cfg(test)]
mod tests;

struct ScheduleHandle {
  dispatcher: DispatcherShared,
}

impl ScheduleHandle {
  const fn new(dispatcher: DispatcherShared) -> Self {
    Self { dispatcher }
  }

  fn schedule(&mut self) {
    // dispatcher.clone() は軽量ハンドルなのでロック外で使う
    let dispatcher = self.dispatcher.clone();
    dispatcher.register_for_execution(ScheduleHints {
      has_system_messages: false,
      has_user_messages:   true,
      backpressure_active: false,
    });
  }
}

struct ScheduleShared {
  inner: ArcShared<RuntimeMutex<ScheduleHandle>>,
}

impl ScheduleShared {
  fn new(dispatcher: DispatcherShared) -> Self {
    let handle = ScheduleHandle::new(dispatcher);
    let inner = ArcShared::new(RuntimeMutex::new(handle));
    Self { inner }
  }

  fn schedule(&self) {
    self.inner.lock().schedule();
  }
}

/// Helper for creating a [`Waker`] that reschedules the dispatcher.
pub(crate) struct ScheduleWaker {
  _marker: PhantomData<()>,
}

impl ScheduleWaker {
  /// Creates a waker that schedules the dispatcher using the provided dispatcher handle.
  pub(crate) fn into_waker(dispatcher: DispatcherShared) -> Waker {
    let shared = ArcShared::new(ScheduleShared::new(dispatcher));
    unsafe { Waker::from_raw(Self::raw_waker(shared)) }
  }

  unsafe fn raw_waker(shared: ArcShared<ScheduleShared>) -> RawWaker {
    let data = ArcShared::into_raw(shared) as *const ();
    RawWaker::new(data, &ScheduleWakerVtable::VTABLE)
  }

  unsafe fn clone(ptr: *const ()) -> RawWaker {
    let shared = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared) };
    let clone = shared.clone();
    let _ = ArcShared::into_raw(shared);
    unsafe { Self::raw_waker(clone) }
  }

  unsafe fn wake(ptr: *const ()) {
    let shared = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared) };
    shared.schedule();
  }

  unsafe fn wake_by_ref(ptr: *const ()) {
    let shared = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared) };
    shared.schedule();
    let _ = ArcShared::into_raw(shared);
  }

  unsafe fn drop(ptr: *const ()) {
    let _ = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared) };
  }
}

struct ScheduleWakerVtable(PhantomData<()>);

impl ScheduleWakerVtable {
  const VTABLE: RawWakerVTable =
    RawWakerVTable::new(ScheduleWaker::clone, ScheduleWaker::wake, ScheduleWaker::wake_by_ref, ScheduleWaker::drop);
}
