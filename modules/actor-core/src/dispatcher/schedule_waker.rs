//! Helper utilities for constructing dispatcher-driven wakers.

use core::{marker::PhantomData, task::{RawWaker, RawWakerVTable, Waker}};

use cellactor_utils_core_rs::sync::ArcShared;

use super::{dispatcher_core::DispatcherCore, dispatcher_struct::Dispatcher};
use crate::RuntimeToolbox;

struct ScheduleHandle<TB: RuntimeToolbox + 'static> {
  dispatcher: ArcShared<DispatcherCore<TB>>,
}

impl<TB: RuntimeToolbox + 'static> ScheduleHandle<TB> {
  const fn new(dispatcher: ArcShared<DispatcherCore<TB>>) -> Self {
    Self { dispatcher }
  }

  fn schedule(&self) {
    Dispatcher::from_core(self.dispatcher.clone()).schedule();
  }
}

/// Helper for creating a [`Waker`] that reschedules the dispatcher.
pub struct ScheduleWaker<TB: RuntimeToolbox + 'static> {
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> ScheduleWaker<TB> {
  /// Creates a waker that schedules the dispatcher using the provided core reference.
  pub fn into_waker(dispatcher: ArcShared<DispatcherCore<TB>>) -> Waker {
    let handle = ArcShared::new(ScheduleHandle::new(dispatcher));
    unsafe { Waker::from_raw(Self::raw_waker(handle)) }
  }

  unsafe fn raw_waker(handle: ArcShared<ScheduleHandle<TB>>) -> RawWaker {
    let data = ArcShared::into_raw(handle) as *const ();
    RawWaker::new(data, &ScheduleWakerVtable::<TB>::VTABLE)
  }

  unsafe fn clone(ptr: *const ()) -> RawWaker {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ScheduleHandle<TB>) };
    let clone = handle.clone();
    let _ = ArcShared::into_raw(handle);
    unsafe { Self::raw_waker(clone) }
  }

  unsafe fn wake(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ScheduleHandle<TB>) };
    handle.schedule();
  }

  unsafe fn wake_by_ref(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ScheduleHandle<TB>) };
    handle.schedule();
    let _ = ArcShared::into_raw(handle);
  }

  unsafe fn drop(ptr: *const ()) {
    let _ = unsafe { ArcShared::from_raw(ptr as *const ScheduleHandle<TB>) };
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
