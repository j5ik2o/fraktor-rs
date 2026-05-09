//! Waker utilities for resuming context pipe tasks.

use core::{
  marker::PhantomData,
  task::{RawWaker, RawWakerVTable, Waker},
};

use fraktor_utils_core_rs::sync::ArcShared;

use crate::actor::context_pipe::context_pipe_waker_handle_shared::ContextPipeWakerHandleShared;

#[cfg(test)]
mod tests;

/// Helper that transforms system references into [`Waker`] instances.
pub(crate) struct ContextPipeWaker {
  _marker: PhantomData<()>,
}

impl ContextPipeWaker {
  /// Creates a waker that notifies the owning actor cell about task readiness.
  pub(crate) fn into_waker(context_pipe_waker_handle_shared: ContextPipeWakerHandleShared) -> Waker {
    let shared = ArcShared::new(context_pipe_waker_handle_shared);
    unsafe { Waker::from_raw(Self::raw_waker(shared)) }
  }

  unsafe fn raw_waker(shared: ArcShared<ContextPipeWakerHandleShared>) -> RawWaker {
    let data = ArcShared::into_raw(shared) as *const ();
    RawWaker::new(data, &ContextPipeWakerVtable::VTABLE)
  }

  unsafe fn clone(ptr: *const ()) -> RawWaker {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerHandleShared) };
    let cloned = handle.clone();
    // Intentionally leak the ArcShared to preserve the reference count; raw_waker took ownership of the
    // clone.
    let _raw = ArcShared::into_raw(handle);
    unsafe { Self::raw_waker(cloned) }
  }

  unsafe fn wake(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerHandleShared) };
    handle.wake();
  }

  unsafe fn wake_by_ref(ptr: *const ()) {
    let handle = unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerHandleShared) };
    handle.wake();
    // Intentionally leak the ArcShared to prevent deallocation; ownership returns to the raw pointer.
    let _raw = ArcShared::into_raw(handle);
  }

  unsafe fn drop(ptr: *const ()) {
    // ArcShared::from_raw で参照カウントを戻し、戻り値は即座に drop して解放する。
    drop(unsafe { ArcShared::from_raw(ptr as *const ContextPipeWakerHandleShared) });
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
