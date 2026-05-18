use core::{
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  time::Duration,
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
  let mut provider = ManualDelayProvider::new();
  let mut future = provider.delay(Duration::from_millis(10));
  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);

  assert!(matches!(Pin::new(&mut future).poll(&mut cx), Poll::Pending));
  assert!(provider.trigger_next());
  assert!(matches!(Pin::new(&mut future).poll(&mut cx), Poll::Ready(())));
}

#[test]
fn trigger_all_completes_every_future() {
  let mut provider = ManualDelayProvider::new();
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
