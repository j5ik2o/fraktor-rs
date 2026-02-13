//! Helper utilities for constructing dispatcher-driven wakers.

use core::{
  marker::PhantomData,
  task::{RawWaker, RawWakerVTable, Waker},
};

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, ToolboxMutex, sync_mutex_family::SyncMutexFamily},
  sync::{ArcShared, sync_mutex_like::SyncMutexLike},
};

use super::dispatcher_shared::DispatcherSharedGeneric;
use crate::core::dispatch::mailbox::ScheduleHints;

#[cfg(test)]
mod tests;

struct ScheduleHandle<TB: RuntimeToolbox + 'static> {
  dispatcher: DispatcherSharedGeneric<TB>,
}

impl<TB: RuntimeToolbox + 'static> ScheduleHandle<TB> {
  const fn new(dispatcher: DispatcherSharedGeneric<TB>) -> Self {
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

struct ScheduleShared<TB: RuntimeToolbox + 'static> {
  inner: ArcShared<ToolboxMutex<ScheduleHandle<TB>, TB>>,
}

impl<TB: RuntimeToolbox + 'static> ScheduleShared<TB> {
  fn new(dispatcher: DispatcherSharedGeneric<TB>) -> Self {
    let handle = ScheduleHandle::new(dispatcher);
    let inner = ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(handle));
    Self { inner }
  }

  fn schedule(&self) {
    self.inner.lock().schedule();
  }
}

/// Helper for creating a [`Waker`] that reschedules the dispatcher.
pub(crate) struct ScheduleWaker<TB: RuntimeToolbox + 'static> {
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> ScheduleWaker<TB> {
  /// Creates a waker that schedules the dispatcher using the provided dispatcher handle.
  pub(crate) fn into_waker(dispatcher: DispatcherSharedGeneric<TB>) -> Waker {
    let shared = ArcShared::new(ScheduleShared::new(dispatcher));
    unsafe { Waker::from_raw(Self::raw_waker(shared)) }
  }

  unsafe fn raw_waker(shared: ArcShared<ScheduleShared<TB>>) -> RawWaker {
    let data = ArcShared::into_raw(shared) as *const ();
    RawWaker::new(data, &ScheduleWakerVtable::<TB>::VTABLE)
  }

  unsafe fn clone(ptr: *const ()) -> RawWaker {
    let shared = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared<TB>) };
    let clone = shared.clone();
    let _ = ArcShared::into_raw(shared);
    unsafe { Self::raw_waker(clone) }
  }

  unsafe fn wake(ptr: *const ()) {
    let shared = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared<TB>) };
    shared.schedule();
  }

  unsafe fn wake_by_ref(ptr: *const ()) {
    let shared = unsafe { ArcShared::from_raw(ptr as *const ScheduleShared<TB>) };
    shared.schedule();
    let _ = ArcShared::into_raw(shared);
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
