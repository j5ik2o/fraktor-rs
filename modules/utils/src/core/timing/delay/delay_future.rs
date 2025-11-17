use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
  time::Duration,
};

use super::{delay_state::DelayState, delay_trigger::DelayTrigger};
use crate::core::sync::ArcShared;

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
