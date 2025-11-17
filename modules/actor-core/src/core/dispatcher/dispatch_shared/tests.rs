use fraktor_utils_rs::core::sync::{ArcShared, shared::Shared};

use super::DispatchShared;
use crate::core::{
  dispatcher::{InlineExecutor, InlineScheduleAdapter, dispatcher_core::DispatcherCore},
  mailbox::Mailbox,
};

#[test]
fn dispatch_shared_new() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let adapter = ArcShared::new(InlineScheduleAdapter::new());
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, adapter, None, None, None));
  let _shared = DispatchShared::new(core.clone());
  assert!(core.with_ref(|_| true));
}

#[test]
fn dispatch_shared_clone() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let adapter = ArcShared::new(InlineScheduleAdapter::new());
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, adapter, None, None, None));
  let shared1 = DispatchShared::new(core.clone());
  let shared2 = shared1.clone();
  assert!(shared1.core.with_ref(|_| true));
  assert!(shared2.core.with_ref(|_| true));
}

#[test]
fn dispatch_shared_drive() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let adapter = ArcShared::new(InlineScheduleAdapter::new());
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, adapter, None, None, None));
  let shared = DispatchShared::new(core);
  shared.drive();
}
