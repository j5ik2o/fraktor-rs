use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  time::Duration,
};

use crate::{
  collections::queue::{DequeBackend, OverflowPolicy, QueueError},
};
use crate::timing::ManualDelayProvider;

const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop_waker);

fn noop_waker() -> Waker {
  unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}

unsafe fn clone(_: *const ()) -> RawWaker {
  RawWaker::new(core::ptr::null(), &VTABLE)
}

unsafe fn wake(_: *const ()) {}

unsafe fn wake_by_ref(_: *const ()) {}

unsafe fn drop_waker(_: *const ()) {}

#[test]
fn deque_offer_future_times_out_when_capacity_never_frees() {
  let backend = DequeBackend::with_capacity(1, OverflowPolicy::Block);
  let _ = backend.offer_back(1);

  let provider = ManualDelayProvider::new();
  let mut future = backend.offer_back_blocking(2).with_timeout(Duration::from_millis(5), &provider);

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert!(matches!(Pin::new(&mut future).poll(&mut context), Poll::Pending));
  assert!(provider.trigger_next());
  let result = Pin::new(&mut future).poll(&mut context);
  assert!(matches!(result, Poll::Ready(Err(QueueError::TimedOut(_)))));
}
