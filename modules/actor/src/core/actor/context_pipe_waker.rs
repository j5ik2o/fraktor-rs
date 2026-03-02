//! Waker utilities for resuming context pipe tasks.

use core::{
  marker::PhantomData,
  task::{RawWaker, RawWakerVTable, Waker},
};

use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

use crate::core::{
  actor::{ContextPipeTaskId, Pid},
  messaging::system_message::SystemMessage,
  system::state::SystemStateShared,
};

#[cfg(test)]
mod tests;

struct ContextPipeWakerHandle {
  system: SystemStateShared,
  pid:    Pid,
  task:   ContextPipeTaskId,
}

impl ContextPipeWakerHandle {
  const fn new(system: SystemStateShared, pid: Pid, task: ContextPipeTaskId) -> Self {
    Self { system, pid, task }
  }

  fn wake(&mut self) {
    // send_system_message は内部でロックを取るため、ロック保持を避けるべくクローン後に実行
    let system = self.system.clone();
    let pid = self.pid;
    let task = self.task;
    if let Err(error) = system.send_system_message(pid, SystemMessage::PipeTask(task)) {
      system.record_send_error(Some(pid), &error);
    }
  }
}

struct ContextPipeWakerShared {
  inner: ArcShared<RuntimeMutex<ContextPipeWakerHandle>>,
}

impl ContextPipeWakerShared {
  fn new(system: SystemStateShared, pid: Pid, task: ContextPipeTaskId) -> Self {
    let handle = ContextPipeWakerHandle::new(system, pid, task);
    let inner = ArcShared::new(RuntimeMutex::new(handle));
    Self { inner }
  }

  fn wake(&self) {
    self.inner.lock().wake();
  }
}

/// Helper that transforms system references into [`Waker`] instances.
pub(crate) struct ContextPipeWaker {
  _marker: PhantomData<()>,
}

impl ContextPipeWaker {
  /// Creates a waker that notifies the owning actor cell about task readiness.
  pub(crate) fn into_waker(system: SystemStateShared, pid: Pid, task: ContextPipeTaskId) -> Waker {
    let shared = ArcShared::new(ContextPipeWakerShared::new(system, pid, task));
    unsafe { Waker::from_raw(Self::raw_waker(shared)) }
  }

  unsafe fn raw_waker(shared: ArcShared<ContextPipeWakerShared>) -> RawWaker {
    let data = ArcShared::into_raw(shared) as *const ();
    RawWaker::new(data, &ContextPipeWakerVtable::VTABLE)
  }

  unsafe fn clone(ptr: *const ()) -> RawWaker {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerShared) };
    let cloned = handle.clone();
    let _ = ArcShared::into_raw(handle);
    unsafe { Self::raw_waker(cloned) }
  }

  unsafe fn wake(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerShared) };
    handle.wake();
  }

  unsafe fn wake_by_ref(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerShared) };
    handle.wake();
    let _ = ArcShared::into_raw(handle);
  }

  unsafe fn drop(ptr: *const ()) {
    let _ = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerShared) };
  }
}

struct ContextPipeWakerVtable(PhantomData<()>);

impl ContextPipeWakerVtable {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(
    ContextPipeWaker::clone,
    ContextPipeWaker::wake,
    ContextPipeWaker::wake_by_ref,
    ContextPipeWaker::drop,
  );
}
