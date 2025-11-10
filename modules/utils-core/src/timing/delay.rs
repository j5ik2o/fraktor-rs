use alloc::vec::Vec;
use core::{
  future::Future,
  pin::Pin,
  sync::atomic::{AtomicBool, Ordering},
  task::{Context, Poll, Waker},
  time::Duration,
};

use spin::Mutex as SpinMutex;

use crate::sync::{ArcShared, NoStdMutex};

/// Provider capable of creating delay futures backed by the current runtime.
pub trait DelayProvider: Send + Sync + 'static {
  /// Returns a future that completes after the specified duration.
  fn delay(&self, duration: Duration) -> DelayFuture;
}

/// Future that resolves once its associated delay has elapsed.
pub struct DelayFuture {
  state:     ArcShared<DelayState>,
  _duration: Duration,
}

impl DelayFuture {
  fn new(duration: Duration) -> (Self, DelayTrigger) {
    let state = ArcShared::new(DelayState::new());
    let trigger = DelayTrigger { state: state.clone() };
    (Self { state, _duration: duration }, trigger)
  }
}

impl Future for DelayFuture {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    if self.state.is_completed() {
      Poll::Ready(())
    } else {
      self.state.register_waker(cx.waker());
      Poll::Pending
    }
  }
}

struct DelayState {
  completed: AtomicBool,
  waker:     SpinMutex<Option<Waker>>,
}

impl DelayState {
  const fn new() -> Self {
    Self { completed: AtomicBool::new(false), waker: SpinMutex::new(None) }
  }

  fn is_completed(&self) -> bool {
    self.completed.load(Ordering::Acquire)
  }

  fn register_waker(&self, waker: &Waker) {
    let mut guard = self.waker.lock();
    match guard.as_ref() {
      | Some(existing) if existing.will_wake(waker) => {},
      | _ => {
        guard.replace(waker.clone());
      },
    }
  }

  fn complete(&self) {
    if self.completed.swap(true, Ordering::Release) {
      return;
    }
    if let Some(waker) = self.waker.lock().take() {
      waker.wake();
    }
  }
}

struct DelayTrigger {
  state: ArcShared<DelayState>,
}

impl DelayTrigger {
  fn fire(&self) {
    self.state.complete();
  }
}

/// Manual provider used in tests to deterministically complete delay futures.
#[derive(Clone)]
pub struct ManualDelayProvider {
  handles: ArcShared<NoStdMutex<Vec<DelayTrigger>>>,
}

impl ManualDelayProvider {
  /// Creates a provider without any scheduled delays.
  #[must_use]
  pub fn new() -> Self {
    Self { handles: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  /// Triggers the next pending delay, returning `true` if a future was completed.
  pub fn trigger_next(&self) -> bool {
    if let Some(handle) = self.handles.lock().pop() {
      handle.fire();
      true
    } else {
      false
    }
  }

  /// Triggers all pending delays.
  pub fn trigger_all(&self) {
    let mut guard = self.handles.lock();
    for handle in guard.drain(..) {
      handle.fire();
    }
  }

  /// Returns the number of pending handles (testing helper).
  pub fn pending_count(&self) -> usize {
    self.handles.lock().len()
  }
}

impl Default for ManualDelayProvider {
  fn default() -> Self {
    Self::new()
  }
}

impl DelayProvider for ManualDelayProvider {
  fn delay(&self, duration: Duration) -> DelayFuture {
    let (future, handle) = DelayFuture::new(duration);
    self.handles.lock().push(handle);
    future
  }
}

#[cfg(test)]
mod tests {
  use core::{
    pin::Pin,
    task::{Context, RawWaker, RawWakerVTable, Waker},
  };

  use super::*;

  fn noop_waker() -> Waker {
    fn clone(_: *const ()) -> RawWaker {
      RawWaker::new(core::ptr::null(), &VTABLE)
    }
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
  }

  #[test]
  fn manual_delay_provider_completes_future_after_trigger() {
    let provider = ManualDelayProvider::new();
    let mut future = provider.delay(Duration::from_millis(10));
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);

    assert!(matches!(Pin::new(&mut future).poll(&mut cx), Poll::Pending));
    assert!(provider.trigger_next());
    assert!(matches!(Pin::new(&mut future).poll(&mut cx), Poll::Ready(())));
  }

  #[test]
  fn trigger_all_completes_every_future() {
    let provider = ManualDelayProvider::new();
    let mut fut1 = provider.delay(Duration::from_millis(5));
    let mut fut2 = provider.delay(Duration::from_millis(10));
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert!(matches!(Pin::new(&mut fut1).poll(&mut cx), Poll::Pending));
    assert!(matches!(Pin::new(&mut fut2).poll(&mut cx), Poll::Pending));

    provider.trigger_all();
    assert!(matches!(Pin::new(&mut fut1).poll(&mut cx), Poll::Ready(())));
    assert!(matches!(Pin::new(&mut fut2).poll(&mut cx), Poll::Ready(())));
  }
}
