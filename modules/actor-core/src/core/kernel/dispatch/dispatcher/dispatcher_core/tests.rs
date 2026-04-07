use alloc::boxed::Box;
use core::time::Duration;

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

use super::DispatcherCore;
use crate::core::kernel::{
  actor::{
    Pid,
    error::ActorError,
    messaging::message_invoker::{MessageInvoker, MessageInvokerShared},
  },
  dispatch::{
    dispatcher::{DispatchExecutorRunner, InlineExecutor, InlineScheduleAdapter},
    mailbox::{Mailbox, metrics_event::MailboxPressureEvent},
  },
};

fn inline_runner() -> ArcShared<DispatchExecutorRunner> {
  ArcShared::new(DispatchExecutorRunner::new(Box::new(InlineExecutor::new())))
}

#[test]
fn dispatcher_core_new() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = inline_runner();
  let adapter = InlineScheduleAdapter::shared();
  let core = DispatcherCore::new(mailbox, executor, adapter, None, None, None);
  let _ = core;
}

#[test]
fn dispatcher_core_new_with_throughput_limit() {
  use core::num::NonZeroUsize;

  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = inline_runner();
  let limit = NonZeroUsize::new(100).unwrap();
  let adapter = InlineScheduleAdapter::shared();
  let core = DispatcherCore::new(mailbox, executor, adapter, Some(limit), None, None);
  let _ = core;
}

#[test]
fn dispatcher_core_executor() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = inline_runner();
  let adapter = InlineScheduleAdapter::shared();
  let core = DispatcherCore::new(mailbox, executor.clone(), adapter, None, None, None);
  let retrieved = core.executor();
  let _ = retrieved;
}

#[test]
fn dispatcher_core_state() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = inline_runner();
  let adapter = InlineScheduleAdapter::shared();
  let core = DispatcherCore::new(mailbox, executor, adapter, None, None, None);
  let state = core.state();
  let _ = state;
}

#[test]
fn dispatcher_core_drive_with_empty_mailbox() {
  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = inline_runner();
  let adapter = InlineScheduleAdapter::shared();
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, adapter, None, None, None));

  DispatcherCore::drive(&core);
}

#[test]
fn dispatcher_core_register_invoker() {
  struct MockInvoker;

  impl MessageInvoker for MockInvoker {
    fn invoke_user_message(
      &mut self,
      _message: crate::core::kernel::actor::messaging::AnyMessage,
    ) -> Result<(), ActorError> {
      Ok(())
    }

    fn invoke_system_message(
      &mut self,
      _message: crate::core::kernel::actor::messaging::system_message::SystemMessage,
    ) -> Result<(), ActorError> {
      Ok(())
    }
  }

  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = inline_runner();
  let adapter = InlineScheduleAdapter::shared();
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, adapter, None, None, None));

  let invoker = MessageInvokerShared::new(Box::new(MockInvoker) as Box<dyn MessageInvoker>);
  core.register_invoker(invoker);
}

#[test]
fn dispatcher_core_invokes_mailbox_pressure_hook_when_full() {
  struct PressureInvoker {
    pressure_calls: ArcShared<NoStdMutex<usize>>,
  }

  impl MessageInvoker for PressureInvoker {
    fn invoke_user_message(
      &mut self,
      _message: crate::core::kernel::actor::messaging::AnyMessage,
    ) -> Result<(), ActorError> {
      Ok(())
    }

    fn invoke_system_message(
      &mut self,
      _message: crate::core::kernel::actor::messaging::system_message::SystemMessage,
    ) -> Result<(), ActorError> {
      Ok(())
    }

    fn invoke_mailbox_pressure(&mut self, _event: &MailboxPressureEvent) -> Result<(), ActorError> {
      *self.pressure_calls.lock() += 1;
      Ok(())
    }
  }

  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = inline_runner();
  let adapter = InlineScheduleAdapter::shared();
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, adapter, None, None, None));

  let pressure_calls = ArcShared::new(NoStdMutex::new(0_usize));
  let invoker = MessageInvokerShared::new(
    Box::new(PressureInvoker { pressure_calls: pressure_calls.clone() }) as Box<dyn MessageInvoker>
  );
  core.register_invoker(invoker);

  let event = MailboxPressureEvent::new(Pid::new(11, 0), 4, 4, 100, Duration::from_millis(1), Some(3));
  DispatcherCore::handle_backpressure(&core, &event);
  DispatcherCore::drive(&core);

  assert_eq!(*pressure_calls.lock(), 1);
}

#[test]
fn dispatcher_core_invokes_mailbox_pressure_hook_when_threshold_is_reached() {
  struct PressureInvoker {
    pressure_calls: ArcShared<NoStdMutex<usize>>,
  }

  impl MessageInvoker for PressureInvoker {
    fn invoke_user_message(
      &mut self,
      _message: crate::core::kernel::actor::messaging::AnyMessage,
    ) -> Result<(), ActorError> {
      Ok(())
    }

    fn invoke_system_message(
      &mut self,
      _message: crate::core::kernel::actor::messaging::system_message::SystemMessage,
    ) -> Result<(), ActorError> {
      Ok(())
    }

    fn invoke_mailbox_pressure(&mut self, _event: &MailboxPressureEvent) -> Result<(), ActorError> {
      *self.pressure_calls.lock() += 1;
      Ok(())
    }
  }

  let mailbox = ArcShared::new(Mailbox::new(crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)));
  let executor = inline_runner();
  let adapter = InlineScheduleAdapter::shared();
  let core = ArcShared::new(DispatcherCore::new(mailbox, executor, adapter, None, None, None));

  let pressure_calls = ArcShared::new(NoStdMutex::new(0_usize));
  let invoker = MessageInvokerShared::new(
    Box::new(PressureInvoker { pressure_calls: pressure_calls.clone() }) as Box<dyn MessageInvoker>
  );
  core.register_invoker(invoker);

  let event = MailboxPressureEvent::new(Pid::new(12, 0), 3, 4, 75, Duration::from_millis(1), Some(3));
  DispatcherCore::handle_backpressure(&core, &event);
  DispatcherCore::drive(&core);

  assert_eq!(*pressure_calls.lock(), 1);
}

#[test]
fn dispatcher_core_prioritizes_system_messages_over_mailbox_pressure() {
  use alloc::vec::Vec;
  use core::num::NonZeroUsize;

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  enum DispatchCall {
    System,
    Pressure,
  }

  struct PriorityInvoker {
    calls: ArcShared<NoStdMutex<Vec<DispatchCall>>>,
  }

  impl MessageInvoker for PriorityInvoker {
    fn invoke_user_message(
      &mut self,
      _message: crate::core::kernel::actor::messaging::AnyMessage,
    ) -> Result<(), ActorError> {
      Ok(())
    }

    fn invoke_system_message(
      &mut self,
      _message: crate::core::kernel::actor::messaging::system_message::SystemMessage,
    ) -> Result<(), ActorError> {
      self.calls.lock().push(DispatchCall::System);
      Ok(())
    }

    fn invoke_mailbox_pressure(&mut self, _event: &MailboxPressureEvent) -> Result<(), ActorError> {
      self.calls.lock().push(DispatchCall::Pressure);
      Ok(())
    }
  }

  let mailbox = ArcShared::new(Mailbox::new(
    crate::core::kernel::dispatch::mailbox::MailboxPolicy::unbounded(None)
      .with_throughput_limit(Some(NonZeroUsize::new(1).unwrap())),
  ));
  let executor = inline_runner();
  let adapter = InlineScheduleAdapter::shared();
  let core = ArcShared::new(DispatcherCore::new(mailbox.clone(), executor, adapter, None, None, None));

  let calls = ArcShared::new(NoStdMutex::new(Vec::new()));
  let invoker =
    MessageInvokerShared::new(Box::new(PriorityInvoker { calls: calls.clone() }) as Box<dyn MessageInvoker>);
  core.register_invoker(invoker);

  mailbox.enqueue_system(crate::core::kernel::actor::messaging::system_message::SystemMessage::Stop).unwrap();
  let event = MailboxPressureEvent::new(Pid::new(13, 0), 4, 4, 100, Duration::from_millis(1), Some(3));
  DispatcherCore::handle_backpressure(&core, &event);
  DispatcherCore::drive(&core);

  let recorded = calls.lock().clone();
  assert_eq!(recorded.len(), 2);
  assert_eq!(recorded[0], DispatchCall::System);
  assert_eq!(recorded[1], DispatchCall::Pressure);
}
