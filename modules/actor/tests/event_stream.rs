#![cfg(not(target_os = "none"))]

extern crate alloc;

use alloc::vec::Vec;
use core::{hint::spin_loop, num::NonZeroUsize};

use fraktor_actor_rs::core::kernel::{
  actor::{
    Actor, ActorContext, ChildRef,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::{MailboxConfig, Props},
  },
  dispatch::mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  event::stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  system::ActorSystem,
};
use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};

struct RecordingSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>,
}

impl RecordingSubscriber {
  fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

struct NullActor;

impl Actor for NullActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
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
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    let child = ctx.spawn_child(&self.child_props).map_err(|_| ActorError::recoverable("spawn failed"))?;
    *self.child_slot.lock() = Some(child);
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[test]
fn dead_letter_event_is_published_when_send_fails() {
  let child_slot = ArcShared::new(NoStdMutex::new(None));

  let mailbox_policy =
    MailboxPolicy::bounded(NonZeroUsize::new(1).expect("non-zero"), MailboxOverflowStrategy::DropNewest, None);
  let mailbox_config = MailboxConfig::new(mailbox_policy);
  let child_props = Props::from_fn(|| NullActor).with_mailbox_config(mailbox_config);

  let props = Props::from_fn({
    let child_slot = child_slot.clone();
    let child_props = child_props.clone();
    move || TestGuardian::new(child_slot.clone(), child_props.clone())
  });
  let tick_driver = fraktor_actor_rs::core::kernel::actor::scheduler::tick_driver::TickDriverConfig::manual(
    fraktor_actor_rs::core::kernel::actor::scheduler::tick_driver::ManualTestDriver::new(),
  );
  let system = ActorSystem::new(&props, tick_driver).expect("system");

  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber::new(events.clone()));
  let _subscription = system.subscribe_event_stream(&subscriber);

  wait_until(|| child_slot.lock().is_some());
  let mut child = child_slot.lock().clone().expect("child");

  child.suspend().expect("suspend child");
  // tell is fire-and-forget; the suspended message is routed to dead letters internally
  child.tell(AnyMessage::new("ping"));

  wait_until(|| !system.dead_letters().is_empty());
  let entries = system.dead_letters();
  assert!(!entries.is_empty());

  wait_until(|| events.lock().iter().any(|event| matches!(event, EventStreamEvent::DeadLetter(_))));

  child.resume().expect("resume child");
  system.terminate().expect("terminate");
  system.run_until_terminated(&fraktor_actor_rs::core::kernel::system::SpinBlocker);
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
