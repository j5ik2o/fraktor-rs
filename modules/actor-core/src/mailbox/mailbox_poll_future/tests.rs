use alloc::format;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use crate::{
  mailbox::{Mailbox, MailboxPolicy},
  messaging::AnyMessage,
};

unsafe fn noop_clone(_: *const ()) -> RawWaker {
  noop_raw_waker()
}

unsafe fn noop_wake(_: *const ()) {}

unsafe fn noop_wake_by_ref(_: *const ()) {}

unsafe fn noop_drop(_: *const ()) {}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop_wake, noop_wake_by_ref, noop_drop);

fn noop_raw_waker() -> RawWaker {
  RawWaker::new(core::ptr::null(), &NOOP_WAKER_VTABLE)
}

fn noop_waker() -> Waker {
  unsafe { Waker::from_raw(noop_raw_waker()) }
}

#[test]
fn mailbox_poll_future_completes_with_message() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));

  // メッセージをエンキュー
  mailbox.enqueue_user(AnyMessage::new(42)).expect("enqueue failed");

  let mut future = mailbox.poll_user_future();

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  let result = Pin::new(&mut future).poll(&mut context);
  match result {
    | Poll::Ready(Ok(message)) => {
      let view = message.as_view();
      assert_eq!(*view.downcast_ref::<i32>().unwrap(), 42);
    },
    | _ => panic!("Expected Poll::Ready(Ok(message))"),
  }
}

#[test]
fn mailbox_poll_future_pending_when_empty() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));

  let mut future = mailbox.poll_user_future();

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  let result = Pin::new(&mut future).poll(&mut context);
  assert!(matches!(result, Poll::Pending));
}

#[test]
fn mailbox_poll_future_debug_format() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let future = mailbox.poll_user_future();

  let debug_str = format!("{:?}", future);
  assert!(debug_str.contains("MailboxPollFuture"));
}
