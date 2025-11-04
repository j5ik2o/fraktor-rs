#![cfg(not(target_os = "none"))]

extern crate alloc;

use alloc::vec::Vec;
use core::{hint::spin_loop, num::NonZeroUsize};

use cellactor_actor_core_rs::{
  NoStdToolbox,
  actor_prim::{Actor, ActorContext},
  error::ActorError,
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  messaging::{AnyMessage, AnyMessageView},
  props::{MailboxConfig, Props},
  system::ActorSystem,
};
use cellactor_utils_core_rs::sync::{ArcShared, NoStdMutex};

struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent<NoStdToolbox>>>>,
}

impl RecordingSubscriber {
  fn new() -> Self {
    Self { events: ArcShared::new(NoStdMutex::new(Vec::new())) }
  }

  fn events(&self) -> Vec<EventStreamEvent<NoStdToolbox>> {
    self.events.lock().clone()
  }
}

impl EventStreamSubscriber<NoStdToolbox> for RecordingSubscriber {
  fn on_event(&self, event: &EventStreamEvent<NoStdToolbox>) {
    self.events.lock().push(event.clone());
  }
}

struct NullActor;

impl Actor<NoStdToolbox> for NullActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContext<'_, NoStdToolbox>,
    _message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn dead_letter_event_is_published_when_send_fails() {
  let props = Props::<NoStdToolbox>::from_fn(|| NullActor);
  let system = ActorSystem::new(&props).expect("system");

  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = system.subscribe_event_stream(&subscriber);

  let mailbox_policy =
    MailboxPolicy::bounded(NonZeroUsize::new(1).expect("non-zero"), MailboxOverflowStrategy::DropNewest, None);
  let mailbox_config = MailboxConfig::new(mailbox_policy);
  let child = system.spawn(&Props::<NoStdToolbox>::from_fn(|| NullActor).with_mailbox(mailbox_config)).expect("spawn");
  let actor_ref = child.actor_ref().clone();

  child.suspend().expect("suspend child");
  let result = actor_ref.tell(AnyMessage::new("ping"));
  assert!(matches!(result, Err(cellactor_actor_core_rs::error::SendError::Suspended(_))));

  wait_until(|| !system.dead_letters().is_empty());
  let entries = system.dead_letters();
  assert!(!entries.is_empty());

  wait_until(|| subscriber_impl.events().iter().any(|event| matches!(event, EventStreamEvent::DeadLetter(_))));

  child.resume().expect("resume child");
  system.terminate().expect("terminate");
  system.run_until_terminated();
}

fn wait_until(condition: impl Fn() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}
