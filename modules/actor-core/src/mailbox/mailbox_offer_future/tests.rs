use alloc::format;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  time::Duration,
};

use fraktor_utils_core_rs::timing::ManualDelayProvider;

use crate::{
  error::SendError,
  mailbox::{EnqueueOutcome, Mailbox, MailboxOverflowStrategy, MailboxPolicy},
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
fn mailbox_offer_future_unbounded_completes_immediately() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let message = AnyMessage::new(42);

  let mut future = mailbox.enqueue_user_future(message);

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  let result = Pin::new(&mut future).poll(&mut context);
  assert!(matches!(result, Poll::Ready(Ok(()))));
}

#[test]
fn mailbox_offer_future_bounded_completes_when_space_available() {
  use core::num::NonZeroUsize;

  let mailbox = Mailbox::new(MailboxPolicy::bounded(
    NonZeroUsize::new(1).unwrap(),
    crate::mailbox::MailboxOverflowStrategy::DropNewest,
    None,
  ));

  let message = AnyMessage::new(42);
  let mut future = mailbox.enqueue_user_future(message);

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  let result = Pin::new(&mut future).poll(&mut context);
  assert!(matches!(result, Poll::Ready(Ok(()))));
}

#[test]
fn mailbox_offer_future_debug_format() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let message = AnyMessage::new(42);
  let future = mailbox.enqueue_user_future(message);

  let debug_str = format!("{:?}", future);
  assert!(debug_str.contains("MailboxOfferFuture"));
}

#[test]
fn mailbox_offer_future_times_out_and_returns_send_error() {
  use core::num::NonZeroUsize;

  let mailbox =
    Mailbox::new(MailboxPolicy::bounded(NonZeroUsize::new(1).unwrap(), MailboxOverflowStrategy::Block, None));

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(0)), Ok(EnqueueOutcome::Enqueued)));

  let provider = ManualDelayProvider::new();
  let mut future = mailbox.enqueue_user_future(AnyMessage::new(1)).with_timeout(Duration::from_millis(5), &provider);

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert!(matches!(Pin::new(&mut future).poll(&mut context), Poll::Pending));
  assert!(provider.trigger_next());
  let result = Pin::new(&mut future).poll(&mut context);
  assert!(matches!(result, Poll::Ready(Err(SendError::Timeout(_)))));
}
