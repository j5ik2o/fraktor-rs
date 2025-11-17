use alloc::boxed::Box;
use core::{
  sync::atomic::{AtomicBool, Ordering},
  task::Waker,
  time::Duration,
};

use spin::Mutex as SpinMutex;

/// Shared state driving delay futures and triggers.
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
