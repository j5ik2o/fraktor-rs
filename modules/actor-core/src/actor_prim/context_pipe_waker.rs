//! Waker utilities for resuming context pipe tasks.

use core::{
  marker::PhantomData,
  task::{RawWaker, RawWakerVTable, Waker},
};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::{
  RuntimeToolbox,
  actor_prim::{ContextPipeTaskId, Pid},
  messaging::SystemMessage,
  system::SystemStateGeneric,
};

#[cfg(test)]
mod tests;

struct ContextPipeWakerShared<TB: RuntimeToolbox + 'static> {
  system: ArcShared<SystemStateGeneric<TB>>,
  pid:    Pid,
  task:   ContextPipeTaskId,
}

impl<TB: RuntimeToolbox + 'static> ContextPipeWakerShared<TB> {
  const fn new(system: ArcShared<SystemStateGeneric<TB>>, pid: Pid, task: ContextPipeTaskId) -> Self {
    Self { system, pid, task }
  }

  fn wake(&self) {
    let _ = self.system.send_system_message(self.pid, SystemMessage::PipeTask(self.task));
  }
}

/// Helper that transforms system references into [`Waker`] instances.
pub(crate) struct ContextPipeWaker<TB: RuntimeToolbox + 'static> {
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> ContextPipeWaker<TB> {
  /// Creates a waker that notifies the owning actor cell about task readiness.
  pub(crate) fn into_waker(system: ArcShared<SystemStateGeneric<TB>>, pid: Pid, task: ContextPipeTaskId) -> Waker {
    let shared = ArcShared::new(ContextPipeWakerShared::new(system, pid, task));
    unsafe { Waker::from_raw(Self::raw_waker(shared)) }
  }

  unsafe fn raw_waker(shared: ArcShared<ContextPipeWakerShared<TB>>) -> RawWaker {
    let data = ArcShared::into_raw(shared) as *const ();
    RawWaker::new(data, &ContextPipeWakerVtable::<TB>::VTABLE)
  }

  unsafe fn clone(ptr: *const ()) -> RawWaker {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerShared<TB>) };
    let cloned = handle.clone();
    let _ = ArcShared::into_raw(handle);
    unsafe { Self::raw_waker(cloned) }
  }

  unsafe fn wake(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerShared<TB>) };
    handle.wake();
  }

  unsafe fn wake_by_ref(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerShared<TB>) };
    handle.wake();
    let _ = ArcShared::into_raw(handle);
  }

  unsafe fn drop(ptr: *const ()) {
    let _ = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerShared<TB>) };
  }
}

struct ContextPipeWakerVtable<TB: RuntimeToolbox + 'static>(PhantomData<TB>);

impl<TB: RuntimeToolbox + 'static> ContextPipeWakerVtable<TB> {
  const VTABLE: RawWakerVTable = RawWakerVTable::new(
    ContextPipeWaker::<TB>::clone,
    ContextPipeWaker::<TB>::wake,
    ContextPipeWaker::<TB>::wake_by_ref,
    ContextPipeWaker::<TB>::drop,
  );
}
