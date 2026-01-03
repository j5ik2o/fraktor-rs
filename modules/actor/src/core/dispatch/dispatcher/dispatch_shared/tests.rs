use alloc::boxed::Box;

use fraktor_utils_rs::core::{
  runtime_toolbox::NoStdToolbox,
  sync::{ArcShared, shared::Shared},
};

use super::DispatchShared;
use crate::core::dispatch::{
  dispatcher::{DispatchExecutorRunnerGeneric, InlineExecutor, InlineScheduleAdapter, dispatcher_core::DispatcherCoreGeneric},
  mailbox::Mailbox,
};

#[test]
fn dispatch_shared_new() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(DispatchExecutorRunnerGeneric::new(Box::new(InlineExecutor::new())));
  let adapter = InlineScheduleAdapter::shared::<NoStdToolbox>();
  let core = ArcShared::new(DispatcherCoreGeneric::new(mailbox, executor, adapter, None, None, None));
  let _shared = DispatchShared::new(core.clone());
  assert!(core.with_ref(|_| true));
}

#[test]
fn dispatch_shared_clone() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(DispatchExecutorRunnerGeneric::new(Box::new(InlineExecutor::new())));
  let adapter = InlineScheduleAdapter::shared::<NoStdToolbox>();
  let core = ArcShared::new(DispatcherCoreGeneric::new(mailbox, executor, adapter, None, None, None));
  let shared1 = DispatchShared::new(core.clone());
  let shared2 = shared1.clone();
  assert!(shared1.core.with_ref(|_| true));
  assert!(shared2.core.with_ref(|_| true));
}

#[test]
fn dispatch_shared_drive() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(DispatchExecutorRunnerGeneric::new(Box::new(InlineExecutor::new())));
  let adapter = InlineScheduleAdapter::shared::<NoStdToolbox>();
  let core = ArcShared::new(DispatcherCoreGeneric::new(mailbox, executor, adapter, None, None, None));
  let shared = DispatchShared::new(core);
  shared.drive();
}
