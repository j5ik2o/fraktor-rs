use alloc::boxed::Box;
use core::{
  future::Future,
  pin::Pin,
  sync::atomic::{AtomicBool, Ordering},
  task::{Context, Poll, Waker},
  time::Duration,
};

use spin::Mutex as SpinMutex;

use crate::{sync::ArcShared, timing::delay::DelayTrigger};

/// Future that resolves once its associated delay has elapsed.
pub struct DelayFuture {
  state: ArcShared<DelayState>,
}

impl DelayFuture {
  /// Creates a future/trigger pair that can be completed externally.
  #[must_use]
  pub fn new_pair(duration: Duration) -> (Self, DelayTrigger) {
    let state = ArcShared::new(DelayState::new(duration));
    let trigger = DelayTrigger::new(state.clone());
    (Self { state }, trigger)
  }
}

impl Drop for DelayFuture {
  fn drop(&mut self) {
    self.state.cancel();
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

pub(crate) struct DelayState {
  completed: AtomicBool,
  waker:     SpinMutex<Option<Waker>>,
  cancel:    SpinMutex<Option<Box<dyn FnOnce() + Send + Sync>>>,
  _duration: Duration,
}

impl DelayState {
  pub(crate) const fn new(duration: Duration) -> Self {
    Self {
      completed: AtomicBool::new(false),
      waker:     SpinMutex::new(None),
      cancel:    SpinMutex::new(None),
      _duration: duration,
    }
  }

  pub(crate) fn is_completed(&self) -> bool {
    self.completed.load(Ordering::Acquire)
  }

  pub(crate) fn register_waker(&self, waker: &Waker) {
    let mut guard = self.waker.lock();
    match guard.as_ref() {
      | Some(existing) if existing.will_wake(waker) => {},
      | _ => {
        guard.replace(waker.clone());
      },
    }
  }

  pub(crate) fn complete(&self) {
    if self.completed.swap(true, Ordering::Release) {
      return;
    }
    self.cancel.lock().take();
    if let Some(waker) = self.waker.lock().take() {
      waker.wake();
    }
  }

  pub(crate) fn install_cancel_hook(&self, hook: Box<dyn FnOnce() + Send + Sync>) {
    self.cancel.lock().replace(hook);
  }

  pub(crate) fn cancel(&self) {
    if self.completed.swap(true, Ordering::AcqRel) {
      return;
    }
    if let Some(hook) = self.cancel.lock().take() {
      hook();
    } else if let Some(waker) = self.waker.lock().take() {
      waker.wake();
    }
  }
}
