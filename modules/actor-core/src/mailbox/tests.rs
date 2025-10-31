use core::{
  num::NonZeroUsize,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::{EnqueueOutcome, Mailbox, MailboxMessage};
use crate::{
  any_message::AnyMessage,
  mailbox_policy::{MailboxOverflowStrategy, MailboxPolicy},
  send_error::SendError,
  system_message::SystemMessage,
};

fn bounded_policy(capacity: usize, overflow: MailboxOverflowStrategy) -> MailboxPolicy {
  MailboxPolicy::bounded(NonZeroUsize::new(capacity).expect("capacity must be > 0"), overflow, None)
}

#[test]
fn drop_newest_returns_error_when_full() {
  let policy = bounded_policy(2, MailboxOverflowStrategy::DropNewest);
  let mailbox = Mailbox::new(policy);

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(1_u32)), Ok(EnqueueOutcome::Enqueued)));
  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(2_u32)), Ok(EnqueueOutcome::Enqueued)));
  let result = mailbox.enqueue_user(AnyMessage::new(3_u32));
  assert!(matches!(result, Err(SendError::Full(_))));
  assert_eq!(mailbox.user_len(), 2);
}

#[test]
fn drop_oldest_replaces_oldest_message() {
  let policy = bounded_policy(2, MailboxOverflowStrategy::DropOldest);
  let mailbox = Mailbox::new(policy);

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(10_u8)), Ok(EnqueueOutcome::Enqueued)));
  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(20_u8)), Ok(EnqueueOutcome::Enqueued)));
  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(30_u8)), Ok(EnqueueOutcome::Enqueued)));

  // The oldest (10) should be removed, leaving 20 and 30.
  if let Some(MailboxMessage::User(msg)) = mailbox.dequeue() {
    assert_eq!(msg.as_view().downcast_ref::<u8>(), Some(&20));
  } else {
    panic!("expected user message");
  }
  if let Some(MailboxMessage::User(msg)) = mailbox.dequeue() {
    assert_eq!(msg.as_view().downcast_ref::<u8>(), Some(&30));
  } else {
    panic!("expected user message");
  }
}

#[test]
fn suspension_blocks_user_messages_but_not_system() {
  let policy = MailboxPolicy::unbounded(None);
  let mailbox = Mailbox::new(policy);

  mailbox.enqueue_system(SystemMessage::Stop).expect("system message");
  assert!(matches!(mailbox.enqueue_user(AnyMessage::new("user")), Ok(EnqueueOutcome::Enqueued)));

  mailbox.suspend();

  match mailbox.dequeue() {
    | Some(MailboxMessage::System(SystemMessage::Stop)) => {},
    | _ => panic!("expected system message while suspended"),
  }

  // While suspended, user messages must not be dequeued.
  assert!(mailbox.dequeue().is_none());

  mailbox.resume();
  match mailbox.dequeue() {
    | Some(MailboxMessage::User(msg)) => {
      assert_eq!(msg.as_view().downcast_ref::<&str>(), Some(&"user"));
    },
    | _ => panic!("expected user message after resume"),
  }
}

#[test]
fn grow_policy_accepts_messages_beyond_initial_capacity() {
  let policy = bounded_policy(2, MailboxOverflowStrategy::Grow);
  let mailbox = Mailbox::new(policy);

  for idx in 0..5 {
    assert!(matches!(mailbox.enqueue_user(AnyMessage::new(idx)), Ok(EnqueueOutcome::Enqueued)));
  }

  assert_eq!(mailbox.user_len(), 5);
}

#[test]
fn block_policy_returns_future_that_completes_on_space() {
  let policy = bounded_policy(1, MailboxOverflowStrategy::Block);
  let mailbox = Mailbox::new(policy);

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(1_u8)), Ok(EnqueueOutcome::Enqueued)));

  let pending = mailbox.enqueue_user(AnyMessage::new(2_u8)).expect("pending result");
  let mut future = match pending {
    | EnqueueOutcome::Pending(fut) => fut,
    | EnqueueOutcome::Enqueued => panic!("expected pending future"),
  };

  let waker = noop_waker();
  let mut cx = Context::from_waker(&waker);
  assert!(matches!(Pin::new(&mut future).poll(&mut cx), Poll::Pending));

  match mailbox.dequeue() {
    | Some(MailboxMessage::User(_)) => {},
    | other => panic!("expected user message, got {:?}", other),
  }

  assert!(matches!(Pin::new(&mut future).poll(&mut cx), Poll::Ready(Ok(()))));
  assert_eq!(mailbox.user_len(), 1);
}

fn noop_waker() -> Waker {
  fn noop_clone(_: *const ()) -> RawWaker {
    RawWaker::new(core::ptr::null(), &VTABLE)
  }
  fn noop(_: *const ()) {}
  static VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);
  unsafe { Waker::from_raw(RawWaker::new(core::ptr::null(), &VTABLE)) }
}
