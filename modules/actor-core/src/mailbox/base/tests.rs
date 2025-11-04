use cellactor_utils_core_rs::sync::ArcShared;

use super::MailboxGeneric;
use crate::{
  NoStdToolbox,
  actor_prim::Pid,
  mailbox::{MailboxInstrumentation, MailboxOverflowStrategy, MailboxPolicy},
  messaging::{AnyMessageGeneric, SystemMessage},
  system::SystemStateGeneric,
};

#[test]
fn mailbox_new() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  let _ = mailbox;
}

#[test]
fn mailbox_set_instrumentation() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  let system_state = ArcShared::new(SystemStateGeneric::<NoStdToolbox>::new());
  let pid = Pid::new(1, 0);
  let instrumentation = MailboxInstrumentation::<NoStdToolbox>::new(system_state, pid, None, None, None);
  mailbox.set_instrumentation(instrumentation);
}

#[test]
fn mailbox_enqueue_system() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  let message = SystemMessage::Stop;
  let result = mailbox.enqueue_system(message);
  assert!(result.is_ok());
}

#[test]
fn mailbox_enqueue_user_unbounded() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  let message = AnyMessageGeneric::new(42_u32);
  let result = mailbox.enqueue_user(message);
  assert!(result.is_ok());
}

#[test]
fn mailbox_enqueue_user_suspended() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  mailbox.suspend();
  let message = AnyMessageGeneric::new(42_u32);
  let result = mailbox.enqueue_user(message);
  assert!(result.is_err());
}

#[test]
fn mailbox_enqueue_user_bounded() {
  use core::num::NonZeroUsize;

  let capacity = NonZeroUsize::new(10).unwrap();
  let policy = MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None);
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(policy);
  let message = AnyMessageGeneric::new(42_u32);
  let result = mailbox.enqueue_user(message);
  assert!(result.is_ok());
}

#[test]
fn mailbox_enqueue_user_future() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  let message = AnyMessageGeneric::new(42_u32);
  let future = mailbox.enqueue_user_future(message);
  drop(future);
}

#[test]
fn mailbox_poll_user_future() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  let future = mailbox.poll_user_future();
  drop(future);
}

#[test]
fn mailbox_dequeue_empty() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  let result = mailbox.dequeue();
  assert!(result.is_none());
}

#[test]
fn mailbox_dequeue_user_message() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  let message = AnyMessageGeneric::new(42_u32);
  mailbox.enqueue_user(message).unwrap();
  let result = mailbox.dequeue();
  assert!(result.is_some());
}

#[test]
fn mailbox_dequeue_system_message_priority() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  let user_message = AnyMessageGeneric::new(1_u32);
  mailbox.enqueue_user(user_message).unwrap();
  let system_message = SystemMessage::Stop;
  mailbox.enqueue_system(system_message).unwrap();

  let result = mailbox.dequeue();
  assert!(result.is_some());
  if let Some(msg) = result {
    assert!(matches!(msg, crate::mailbox::MailboxMessage::System(_)));
  }
}

#[test]
fn mailbox_dequeue_suspended() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  let message = AnyMessageGeneric::new(42_u32);
  mailbox.enqueue_user(message).unwrap();
  mailbox.suspend();
  let result = mailbox.dequeue();
  assert!(result.is_none());
}

#[test]
fn mailbox_suspend_resume() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  assert!(!mailbox.is_suspended());
  mailbox.suspend();
  assert!(mailbox.is_suspended());
  mailbox.resume();
  assert!(!mailbox.is_suspended());
}

#[test]
fn mailbox_user_len() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  assert_eq!(mailbox.user_len(), 0);
  mailbox.enqueue_user(AnyMessageGeneric::new(1_u32)).unwrap();
  assert_eq!(mailbox.user_len(), 1);
  mailbox.enqueue_user(AnyMessageGeneric::new(2_u32)).unwrap();
  assert_eq!(mailbox.user_len(), 2);
  let _ = mailbox.dequeue();
  assert_eq!(mailbox.user_len(), 1);
}

#[test]
fn mailbox_system_len() {
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(MailboxPolicy::unbounded(None));
  assert_eq!(mailbox.system_len(), 0);
  mailbox.enqueue_system(SystemMessage::Stop).unwrap();
  assert_eq!(mailbox.system_len(), 1);
  mailbox.enqueue_system(SystemMessage::Stop).unwrap();
  assert_eq!(mailbox.system_len(), 2);
  let _ = mailbox.dequeue();
  assert_eq!(mailbox.system_len(), 1);
}

#[test]
fn mailbox_throughput_limit() {
  use core::num::NonZeroUsize;

  let limit = NonZeroUsize::new(100).unwrap();
  let policy = MailboxPolicy::unbounded(Some(limit));
  let mailbox = MailboxGeneric::<NoStdToolbox>::new(policy);
  assert_eq!(mailbox.throughput_limit(), Some(limit));

  let policy_no_limit = MailboxPolicy::unbounded(None);
  let mailbox_no_limit = MailboxGeneric::<NoStdToolbox>::new(policy_no_limit);
  assert_eq!(mailbox_no_limit.throughput_limit(), None);
}
