//! Tests for [`TickExecutorSignal`].

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use super::TickExecutorSignal;

fn noop_waker() -> core::task::Waker {
  use core::task::{RawWaker, RawWakerVTable, Waker};

  const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake, drop);

  unsafe fn clone(data: *const ()) -> RawWaker {
    RawWaker::new(data, &VTABLE)
  }
  unsafe fn wake(_data: *const ()) {}
  unsafe fn drop(_data: *const ()) {}

  unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}

fn poll_future<F: Future<Output = ()> + core::marker::Unpin>(future: &mut F) -> Poll<()> {
  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);
  Pin::new(future).poll(&mut cx)
}

#[test]
fn notify_sets_pending_flag_visible_via_arm() {
  let signal = TickExecutorSignal::new();
  assert!(!signal.arm(), "no pending work before notify");
  signal.notify();
  assert!(signal.arm(), "arm should detect pending work");
  assert!(!signal.arm(), "arm should reset pending flag");
}

#[test]
fn wait_async_resolves_after_notify() {
  let signal = TickExecutorSignal::new();
  let mut future = signal.wait_async();
  assert!(matches!(poll_future(&mut future), Poll::Pending));
  signal.notify();
  assert!(matches!(poll_future(&mut future), Poll::Ready(())));
}
