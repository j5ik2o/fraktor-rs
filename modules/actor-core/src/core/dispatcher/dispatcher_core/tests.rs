use fraktor_utils_core_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use super::DispatcherCore;
use crate::core::{
  dispatcher::{InlineExecutor, InlineScheduleAdapter},
  error::ActorError,
  mailbox::Mailbox,
  messaging::message_invoker::MessageInvoker,
};

#[test]
fn dispatcher_core_new() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let adapter = ArcShared::new(InlineScheduleAdapter::new());
  let core = DispatcherCore::new(mailbox, executor, adapter, None, None, None);
  let _ = core;
}

#[test]
fn dispatcher_core_new_with_throughput_limit() {
  use core::num::NonZeroUsize;

  let mailbox = ArcShared::new(Mailbox::new(crate::core::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let limit = NonZeroUsize::new(100).unwrap();
  let adapter = ArcShared::new(InlineScheduleAdapter::new());
  let core = DispatcherCore::new(mailbox, executor, adapter, Some(limit), None, None);
  let _ = core;
}

#[test]
fn dispatcher_core_mailbox() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let adapter = ArcShared::new(InlineScheduleAdapter::new());
  let core = DispatcherCore::new(mailbox.clone(), executor, adapter, None, None, None);
  let retrieved = core.mailbox();
  let _ = retrieved;
}

#[test]
fn dispatcher_core_executor() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let adapter = ArcShared::new(InlineScheduleAdapter::new());
  let core = DispatcherCore::new(mailbox, executor.clone(), adapter, None, None, None);
  let retrieved = core.executor();
  let _ = retrieved;
}

#[test]
fn dispatcher_core_state() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let adapter = ArcShared::new(InlineScheduleAdapter::new());
  let core = DispatcherCore::new(mailbox, executor, adapter, None, None, None);
  let state = core.state();
  let _ = state;
}

#[test]
fn dispatcher_core_drive_with_empty_mailbox() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let adapter = ArcShared::new(InlineScheduleAdapter::new());
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, adapter, None, None, None));

  DispatcherCore::drive(&core);
}

#[test]
fn dispatcher_core_register_invoker() {
  struct MockInvoker;

  impl MessageInvoker<NoStdToolbox> for MockInvoker {
    fn invoke_user_message(&self, _message: crate::core::messaging::AnyMessage) -> Result<(), ActorError> {
      Ok(())
    }

    fn invoke_system_message(&self, _message: crate::core::messaging::SystemMessage) -> Result<(), ActorError> {
      Ok(())
    }
  }

  let mailbox = ArcShared::new(Mailbox::new(crate::core::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::new());
  let adapter = ArcShared::new(InlineScheduleAdapter::new());
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, adapter, None, None, None));

  let invoker = ArcShared::new(MockInvoker);
  core.register_invoker(invoker);
}
