use cellactor_utils_core_rs::sync::{ArcShared, Shared};

use super::DispatchShared;
use crate::{
  dispatcher::{InlineExecutor, dispatcher_core::DispatcherCore},
  mailbox::Mailbox,
};

#[test]
fn dispatch_shared_new() {
  let mailbox = ArcShared::new(Mailbox::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, None, None, None));
  let _shared = DispatchShared::new(core.clone());
  assert!(core.with_ref(|_| true));
}

#[test]
fn dispatch_shared_clone() {
  let mailbox = ArcShared::new(Mailbox::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, None, None, None));
  let shared1 = DispatchShared::new(core.clone());
  let shared2 = shared1.clone();
  assert!(shared1.core.with_ref(|_| true));
  assert!(shared2.core.with_ref(|_| true));
}

#[test]
fn dispatch_shared_drive() {
  let mailbox = ArcShared::new(Mailbox::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, None, None, None));
  let shared = DispatchShared::new(core);
  shared.drive();
}
