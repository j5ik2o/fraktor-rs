use core::task::{RawWaker, RawWakerVTable, Waker};

use cellactor_utils_core_rs::sync::ArcShared;

use super::{dispatcher_core::DispatcherCore, dispatcher_struct::Dispatcher};

struct ScheduleHandle {
  dispatcher: ArcShared<DispatcherCore>,
}

impl ScheduleHandle {
  fn new(dispatcher: ArcShared<DispatcherCore>) -> Self {
    Self { dispatcher }
  }

  fn schedule(&self) {
    Dispatcher::from_core(self.dispatcher.clone()).schedule();
  }
}

/// ディスパッチャ実行を喚起する Waker を生成する補助。
pub struct ScheduleWaker;

impl ScheduleWaker {
  /// ディスパッチャを再度スケジューリングする Waker を生成する。
  pub fn into_waker(dispatcher: ArcShared<DispatcherCore>) -> Waker {
    let handle = ArcShared::new(ScheduleHandle::new(dispatcher));
    unsafe { Waker::from_raw(Self::raw_waker(handle)) }
  }

  unsafe fn raw_waker(handle: ArcShared<ScheduleHandle>) -> RawWaker {
    let data = ArcShared::into_raw(handle) as *const ();
    RawWaker::new(data, &VTABLE)
  }

  unsafe fn clone(ptr: *const ()) -> RawWaker {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ScheduleHandle) };
    let clone = handle.clone();
    let _ = ArcShared::into_raw(handle);
    unsafe { Self::raw_waker(clone) }
  }

  unsafe fn wake(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ScheduleHandle) };
    handle.schedule();
  }

  unsafe fn wake_by_ref(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ScheduleHandle) };
    handle.schedule();
    let _ = ArcShared::into_raw(handle);
  }

  unsafe fn drop(ptr: *const ()) {
    let _ = unsafe { ArcShared::from_raw(ptr as *const ScheduleHandle) };
  }
}

static VTABLE: RawWakerVTable =
  RawWakerVTable::new(ScheduleWaker::clone, ScheduleWaker::wake, ScheduleWaker::wake_by_ref, ScheduleWaker::drop);
