#![cfg(not(target_os = "none"))]

extern crate alloc;

use alloc::vec::Vec;
use core::{hint::spin_loop, num::NonZeroUsize};

use fraktor_actor_core_rs::{
  NoStdToolbox,
  actor_prim::{Actor, ActorContextGeneric, ChildRef},
  error::ActorError,
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  messaging::{AnyMessage, AnyMessageView},
  props::{MailboxConfig, Props},
  system::ActorSystem,
};
use fraktor_utils_core_rs::sync::{ArcShared, NoStdMutex};

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

impl Actor for NullActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

struct TestGuardian {
  child_slot:  ArcShared<NoStdMutex<Option<ChildRef>>>,
  child_props: Props,
}

impl TestGuardian {
  fn new(child_slot: ArcShared<NoStdMutex<Option<ChildRef>>>, child_props: Props) -> Self {
    Self { child_slot, child_props }
  }
}

impl Actor for TestGuardian {
  fn pre_start(&mut self, ctx: &mut ActorContextGeneric<'_, NoStdToolbox>) -> Result<(), ActorError> {
    let child = ctx.spawn_child(&self.child_props).map_err(|_| ActorError::recoverable("spawn failed"))?;
    *self.child_slot.lock() = Some(child);
    Ok(())
  }

  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageView<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn dead_letter_event_is_published_when_send_fails() {
  let child_slot = ArcShared::new(NoStdMutex::new(None));

  let mailbox_policy =
    MailboxPolicy::bounded(NonZeroUsize::new(1).expect("non-zero"), MailboxOverflowStrategy::DropNewest, None);
  let mailbox_config = MailboxConfig::new(mailbox_policy);
  let child_props = Props::from_fn(|| NullActor).with_mailbox(mailbox_config);

  let props = Props::from_fn({
    let child_slot = child_slot.clone();
    let child_props = child_props.clone();
    move || TestGuardian::new(child_slot.clone(), child_props.clone())
  });
  let system = ActorSystem::new(&props).expect("system");

  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = system.subscribe_event_stream(&subscriber);

  wait_until(|| child_slot.lock().is_some());
  let child = child_slot.lock().clone().expect("child");
  let actor_ref = child.actor_ref().clone();

  child.suspend().expect("suspend child");
  let result = actor_ref.tell(AnyMessage::new("ping"));
  assert!(matches!(result, Err(fraktor_actor_core_rs::error::SendError::Suspended(_))));

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
