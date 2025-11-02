use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

use cellactor_utils_core_rs::sync::ArcShared;

use super::{dispatch_executor::DispatchExecutor, dispatch_handle::DispatchHandle, dispatcher_struct::Dispatcher};
use crate::{
  ActorError, ActorRefSender, ActorRuntimeMutex,
  any_message::AnyMessage,
  mailbox::Mailbox,
  mailbox_policy::{MailboxOverflowStrategy, MailboxPolicy},
  message_invoker::MessageInvoker,
  system_message::SystemMessage,
};

struct RecordingExecutor {
  tasks: ActorRuntimeMutex<Vec<DispatchHandle>>,
  runs:  AtomicUsize,
}

impl RecordingExecutor {
  fn new() -> Self {
    Self { tasks: ActorRuntimeMutex::new(Vec::new()), runs: AtomicUsize::new(0) }
  }

  fn drain(&self) {
    loop {
      let task = { self.tasks.lock().pop() };
      match task {
        | Some(dispatcher) => {
          self.runs.fetch_add(1, Ordering::Relaxed);
          dispatcher.drive();
        },
        | None => break,
      }
    }
  }
}

impl DispatchExecutor for RecordingExecutor {
  fn execute(&self, dispatcher: DispatchHandle) {
    self.tasks.lock().push(dispatcher);
  }
}

#[derive(Clone)]
enum RecordedMessage {
  User(AnyMessage),
  System(SystemMessage),
}

struct RecordingInvoker {
  events: ActorRuntimeMutex<Vec<RecordedMessage>>,
}

impl RecordingInvoker {
  fn new() -> Self {
    Self { events: ActorRuntimeMutex::new(Vec::new()) }
  }

  fn take_events(&self) -> Vec<RecordedMessage> {
    self.events.lock().clone()
  }
}

impl MessageInvoker for RecordingInvoker {
  fn invoke_user_message(&self, message: AnyMessage) -> Result<(), ActorError> {
    self.events.lock().push(RecordedMessage::User(message));
    Ok(())
  }

  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError> {
    self.events.lock().push(RecordedMessage::System(message));
    Ok(())
  }
}

#[test]
fn processes_user_messages_via_sender() {
  let policy = MailboxPolicy::unbounded(None);
  let mailbox = ArcShared::new(Mailbox::new(policy));
  let executor = ArcShared::new(RecordingExecutor::new());
  let dispatcher = Dispatcher::new(mailbox, executor.clone());
  let invoker = ArcShared::new(RecordingInvoker::new());
  dispatcher.register_invoker(invoker.clone());

  let sender = dispatcher.into_sender();
  sender.send(AnyMessage::new(42_u32)).expect("enqueue");
  executor.drain();

  let events = invoker.take_events();
  assert_eq!(events.len(), 1);
  match &events[0] {
    | RecordedMessage::User(message) => {
      assert_eq!(message.as_view().downcast_ref::<u32>(), Some(&42));
    },
    | RecordedMessage::System(_) => panic!("expected user message"),
  }
}

#[test]
fn system_messages_take_priority() {
  let policy = MailboxPolicy::unbounded(None);
  let mailbox = ArcShared::new(Mailbox::new(policy));
  let executor = ArcShared::new(RecordingExecutor::new());
  let dispatcher = Dispatcher::new(mailbox, executor.clone());
  let invoker = ArcShared::new(RecordingInvoker::new());
  dispatcher.register_invoker(invoker.clone());

  let sender = dispatcher.into_sender();
  sender.send(AnyMessage::new(1_u8)).expect("enqueue user");
  dispatcher.enqueue_system(SystemMessage::Stop).expect("enqueue system");
  executor.drain();

  let events = invoker.take_events();
  assert_eq!(events.len(), 2);
  match &events[0] {
    | RecordedMessage::System(SystemMessage::Stop) => {},
    | _ => panic!("system message should be processed first"),
  }
}

#[test]
fn throughput_limit_reschedules() {
  use core::num::NonZeroUsize;

  let policy = MailboxPolicy::bounded(NonZeroUsize::new(1).unwrap(), MailboxOverflowStrategy::Grow, None)
    .with_throughput_limit(Some(NonZeroUsize::new(1).unwrap()));
  let mailbox = ArcShared::new(Mailbox::new(policy));
  let executor = ArcShared::new(RecordingExecutor::new());
  let dispatcher = Dispatcher::new(mailbox, executor.clone());
  let invoker = ArcShared::new(RecordingInvoker::new());
  dispatcher.register_invoker(invoker.clone());

  let sender = dispatcher.into_sender();
  sender.send(AnyMessage::new(10_u8)).expect("first message");
  sender.send(AnyMessage::new(20_u8)).expect("second message");
  executor.drain();

  let events = invoker.take_events();
  assert_eq!(events.len(), 2);
}
