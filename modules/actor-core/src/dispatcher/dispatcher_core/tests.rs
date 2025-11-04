use cellactor_utils_core_rs::sync::ArcShared;

use super::DispatcherCore;
use crate::{NoStdToolbox, dispatcher::InlineExecutor, mailbox::MailboxGeneric};

#[test]
fn dispatcher_core_new() {
  let mailbox = ArcShared::new(MailboxGeneric::<NoStdToolbox>::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = DispatcherCore::<NoStdToolbox>::new(mailbox, executor, None);
  let _ = core;
}

#[test]
fn dispatcher_core_new_with_throughput_limit() {
  use core::num::NonZeroUsize;

  let mailbox = ArcShared::new(MailboxGeneric::<NoStdToolbox>::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let limit = NonZeroUsize::new(100).unwrap();
  let core = DispatcherCore::<NoStdToolbox>::new(mailbox, executor, Some(limit));
  let _ = core;
}

#[test]
fn dispatcher_core_mailbox() {
  let mailbox = ArcShared::new(MailboxGeneric::<NoStdToolbox>::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = DispatcherCore::<NoStdToolbox>::new(mailbox.clone(), executor, None);
  let retrieved = core.mailbox();
  let _ = retrieved;
}

#[test]
fn dispatcher_core_executor() {
  let mailbox = ArcShared::new(MailboxGeneric::<NoStdToolbox>::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = DispatcherCore::<NoStdToolbox>::new(mailbox, executor.clone(), None);
  let retrieved = core.executor();
  let _ = retrieved;
}

#[test]
fn dispatcher_core_state() {
  let mailbox = ArcShared::new(MailboxGeneric::<NoStdToolbox>::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = DispatcherCore::<NoStdToolbox>::new(mailbox, executor, None);
  let state = core.state();
  let _ = state;
}

#[test]
fn dispatcher_core_drive_with_empty_mailbox() {
  let mailbox = ArcShared::new(MailboxGeneric::<NoStdToolbox>::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = ArcShared::new(DispatcherCore::<NoStdToolbox>::new(mailbox, executor, None));

  DispatcherCore::drive(&core);
}

#[test]
fn dispatcher_core_register_invoker() {
  use crate::{error::ActorError, messaging::message_invoker::MessageInvoker};

  struct MockInvoker;

  impl MessageInvoker<NoStdToolbox> for MockInvoker {
    fn invoke_user_message(
      &self,
      _message: crate::messaging::AnyMessageGeneric<NoStdToolbox>,
    ) -> Result<(), ActorError> {
      Ok(())
    }

    fn invoke_system_message(&self, _message: crate::messaging::SystemMessage) -> Result<(), ActorError> {
      Ok(())
    }
  }

  let mailbox = ArcShared::new(MailboxGeneric::<NoStdToolbox>::new(crate::mailbox::MailboxPolicy::unbounded(None)));
  let executor = ArcShared::new(InlineExecutor::<NoStdToolbox>::new());
  let core = ArcShared::new(DispatcherCore::<NoStdToolbox>::new(mailbox, executor, None));

  let invoker = ArcShared::new(MockInvoker);
  core.register_invoker(invoker);
}
