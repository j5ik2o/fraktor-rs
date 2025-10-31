use core::{
  pin::Pin,
  sync::atomic::{AtomicBool, Ordering},
  task::{Context, RawWaker, RawWakerVTable, Waker},
};

use cellactor_utils_core_rs::sync::ArcShared;

use super::ActorFuture;

unsafe fn waker_clone(ptr: *const ()) -> RawWaker {
  let arc = unsafe { ArcShared::from_raw(ptr.cast::<AtomicBool>()) };
  let cloned = arc.clone();
  let _ = arc.into_raw();
  let raw = cloned.into_raw().cast::<()>();
  RawWaker::new(raw, &VTABLE)
}

unsafe fn waker_wake(ptr: *const ()) {
  let arc = unsafe { ArcShared::from_raw(ptr.cast::<AtomicBool>()) };
  arc.store(true, Ordering::SeqCst);
}

unsafe fn waker_wake_by_ref(ptr: *const ()) {
  let arc = unsafe { ArcShared::from_raw(ptr.cast::<AtomicBool>()) };
  arc.store(true, Ordering::SeqCst);
  let _ = arc.into_raw();
}

unsafe fn waker_drop(ptr: *const ()) {
  let _ = unsafe { ArcShared::from_raw(ptr.cast::<AtomicBool>()) };
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(waker_clone, waker_wake, waker_wake_by_ref, waker_drop);

fn test_waker(flag: ArcShared<AtomicBool>) -> Waker {
  let raw = flag.into_raw().cast::<()>();
  unsafe { Waker::from_raw(RawWaker::new(raw, &VTABLE)) }
}

#[test]
fn completes_once_and_returns_first_value() {
  let future = ActorFuture::new();

  future.complete(10_u32);
  future.complete(99_u32);

  assert_eq!(future.try_take(), Some(10_u32));
  assert_eq!(future.try_take(), None);
}

#[test]
fn listener_receives_wake_on_completion() {
  let future = ActorFuture::new();
  let mut listener = future.listener();

  let flag = ArcShared::new(AtomicBool::new(false));
  let waker = test_waker(flag.clone());
  let mut cx = Context::from_waker(&waker);

  assert!(matches!(Pin::new(&mut listener).poll(&mut cx), core::task::Poll::Pending));
  assert!(!flag.load(Ordering::SeqCst));

  future.complete(7_u8);

  assert!(flag.load(Ordering::SeqCst));
  assert!(matches!(Pin::new(&mut listener).poll(&mut cx), core::task::Poll::Ready(7_u8)));
}
