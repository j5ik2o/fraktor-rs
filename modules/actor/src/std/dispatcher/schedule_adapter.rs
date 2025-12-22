extern crate std;

use std::{
  any::Any,
  sync::atomic::{AtomicUsize, Ordering},
  task::{RawWaker, RawWakerVTable, Waker},
  thread,
};

use fraktor_utils_rs::{core::sync::ArcShared, std::runtime_toolbox::StdToolbox};

use crate::{
  core::{dispatcher::ScheduleAdapter, mailbox::ScheduleHints},
  std::dispatcher::DispatcherShared,
};

#[cfg(test)]
mod tests;

/// Schedule adapter optimised for standard (std) runtimes.
pub struct StdScheduleAdapter {
  pending_calls:  AtomicUsize,
  rejected_calls: AtomicUsize,
}

impl StdScheduleAdapter {
  /// Creates a new adapter with zeroed counters.
  #[must_use]
  pub const fn new() -> Self {
    Self { pending_calls: AtomicUsize::new(0), rejected_calls: AtomicUsize::new(0) }
  }
}

impl Default for StdScheduleAdapter {
  fn default() -> Self {
    Self::new()
  }
}

impl ScheduleAdapter<StdToolbox> for StdScheduleAdapter {
  fn create_waker(&mut self, dispatcher: DispatcherShared) -> Waker {
    StdScheduleWaker::into_waker(dispatcher)
  }

  fn on_pending(&mut self) {
    self.pending_calls.fetch_add(1, Ordering::Relaxed);
    thread::yield_now();
  }

  fn notify_rejected(&mut self, _attempts: usize) {
    self.rejected_calls.fetch_add(1, Ordering::Relaxed);
    thread::yield_now();
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}

struct StdScheduleShared {
  dispatcher: DispatcherShared,
}

impl StdScheduleShared {
  const fn new(dispatcher: DispatcherShared) -> Self {
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

struct StdScheduleWaker;

impl StdScheduleWaker {
  fn into_waker(dispatcher: DispatcherShared) -> Waker {
    let shared = ArcShared::new(StdScheduleShared::new(dispatcher));
    unsafe { Waker::from_raw(Self::raw_waker(shared)) }
  }

  unsafe fn raw_waker(shared: ArcShared<StdScheduleShared>) -> RawWaker {
    let data = ArcShared::into_raw(shared) as *const ();
    RawWaker::new(data, &StdScheduleWakerVtable::VTABLE)
  }

  unsafe fn clone(ptr: *const ()) -> RawWaker {
    let shared = unsafe { ArcShared::from_raw(ptr as *const StdScheduleShared) };
    let cloned = shared.clone();
    let _ = ArcShared::into_raw(shared);
    unsafe { Self::raw_waker(cloned) }
  }

  unsafe fn wake(ptr: *const ()) {
    let shared = unsafe { ArcShared::from_raw(ptr as *const StdScheduleShared) };
    shared.schedule();
  }

  unsafe fn wake_by_ref(ptr: *const ()) {
    let shared = unsafe { ArcShared::from_raw(ptr as *const StdScheduleShared) };
    shared.schedule();
    let _ = ArcShared::into_raw(shared);
  }

  unsafe fn drop(ptr: *const ()) {
    let _ = unsafe { ArcShared::from_raw(ptr as *const StdScheduleShared) };
  }
}

struct StdScheduleWakerVtable;

impl StdScheduleWakerVtable {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(
    StdScheduleWaker::clone,
    StdScheduleWaker::wake,
    StdScheduleWaker::wake_by_ref,
    StdScheduleWaker::drop,
  );
}
