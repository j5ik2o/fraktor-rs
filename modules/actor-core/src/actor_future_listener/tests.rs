use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use crate::{NoStdToolbox, actor_future::ActorFuture};

fn noop_waker() -> Waker {
  fn noop(_: *const ()) {}
  fn clone(_: *const ()) -> RawWaker {
    RawWaker::new(core::ptr::null(), &VTABLE)
  }
  static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
  unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}

#[test]
fn listener_polls_underlying_future() {
  let future: ActorFuture<u8, NoStdToolbox> = ActorFuture::new();
  let mut listener = future.listener();
  future.complete(1_u8);

  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);
  assert_eq!(Pin::new(&mut listener).poll(&mut cx), Poll::Ready(1));
}
