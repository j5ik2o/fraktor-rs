use core::{
  future::Future,
  pin::Pin,
  sync::atomic::{AtomicBool, Ordering},
  task::{Context, Poll, Waker},
  time::Duration,
};

use spin::Mutex as SpinMutex;

use crate::sync::ArcShared;

/// Future that resolves once its associated delay has elapsed.
pub struct DelayFuture {
  state: ArcShared<DelayState>,
}

impl DelayFuture {
  pub(super) fn new(duration: Duration) -> (Self, DelayTrigger) {
    let state = ArcShared::new(DelayState::new(duration));
    let trigger = DelayTrigger { state: state.clone() };
    (Self { state }, trigger)
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
  _duration: Duration,
}

impl DelayState {
  const fn new(duration: Duration) -> Self {
    Self { completed: AtomicBool::new(false), waker: SpinMutex::new(None), _duration: duration }
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

/// Handle owned by providers to complete a delay.
pub(super) struct DelayTrigger {
  state: ArcShared<DelayState>,
}

impl DelayTrigger {
  pub(super) fn fire(&self) {
    self.state.complete();
  }
}
