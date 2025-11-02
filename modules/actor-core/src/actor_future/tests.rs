#![cfg(test)]

use core::{future::Future, pin::Pin, task::{Context, Poll, RawWaker, RawWakerVTable, Waker}};

use crate::actor_future::ActorFuture;

fn noop_waker() -> Waker {
  fn noop(_: *const ()) {}
  fn clone(_: *const ()) -> RawWaker {
    RawWaker::new(core::ptr::null(), &VTABLE)
  }
  static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
  unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}

#[test]
fn completes_and_listens() {
  let future = ActorFuture::new();
  let mut listener = future.listener();

  assert!(future.try_take().is_none());

  future.complete(10);

  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);
  match Pin::new(&mut listener).poll(&mut cx) {
    | Poll::Ready(value) => assert_eq!(value, 10),
    | Poll::Pending => panic!("future should be ready"),
  }
}
