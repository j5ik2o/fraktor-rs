extern crate alloc;

use alloc::{string::String, vec::Vec};
use core::{hint::spin_loop, num::NonZeroUsize};

use fraktor_utils_core_rs::core::{
  runtime_toolbox::{NoStdMutex, NoStdToolbox},
  sync::ArcShared,
};

use crate::core::{
  actor_prim::{Actor, ActorContextGeneric},
  error::ActorError,
  event_stream::{EventStreamEvent, EventStreamSubscriber},
  logging::LogLevel,
  mailbox::{Mailbox, MailboxOverflowStrategy, MailboxPolicy, ScheduleHints},
  messaging::{AnyMessage, AnyMessageViewGeneric, SystemMessage},
  props::{MailboxConfig, Props},
  system::ActorSystem,
};

struct PassiveActor;

impl Actor for PassiveActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContextGeneric<'_, NoStdToolbox>,
    _message: AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

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

#[test]
fn mailbox_metrics_and_warnings_are_emitted() {
  let warn_threshold = NonZeroUsize::new(1).unwrap();
  let capacity = NonZeroUsize::new(2).unwrap();
  let mailbox_config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_warn_threshold(Some(warn_threshold));
  let props = Props::from_fn(|| PassiveActor).with_mailbox(mailbox_config);
  let system = ActorSystem::new(&props).expect("system");

  let subscriber_impl = ArcShared::new(RecordingSubscriber::new());
  let subscriber: ArcShared<dyn EventStreamSubscriber<NoStdToolbox>> = subscriber_impl.clone();
  let _subscription = system.subscribe_event_stream(&subscriber);

  system.user_guardian_ref().tell(AnyMessage::new("first")).expect("send");

  wait_until(|| {
    let events = subscriber_impl.events();
    let has_metrics = events.iter().any(|event| matches!(event, EventStreamEvent::Mailbox(_)));
    let has_warning =
      events.iter().any(|event| matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Warn));
    has_metrics && has_warning
  });

  system.terminate().expect("terminate");
  system.run_until_terminated();
}

#[test]
fn mailbox_schedule_requests_follow_state_engine() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let hints = ScheduleHints { has_system_messages: true, has_user_messages: false, backpressure_active: false };

  assert!(mailbox.request_schedule(hints));
  assert!(!mailbox.request_schedule(hints));

  mailbox.set_running();
  assert!(!mailbox.request_schedule(hints));

  let _ = mailbox.set_idle();
  assert!(mailbox.request_schedule(hints));
}

#[test]
fn mailbox_schedule_hints_reflect_current_workload() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));

  let idle_hints = mailbox.current_schedule_hints();
  assert!(!idle_hints.has_system_messages);
  assert!(!idle_hints.has_user_messages);
  assert!(!idle_hints.backpressure_active);

  mailbox.enqueue_system(SystemMessage::Create).expect("system enqueue");
  let system_hints = mailbox.current_schedule_hints();
  assert!(system_hints.has_system_messages);
  assert!(!system_hints.has_user_messages);
  assert!(!system_hints.backpressure_active);
  let _ = mailbox.dequeue();

  mailbox.enqueue_user(AnyMessage::new(String::from("user"))).expect("user enqueue");
  let user_hints = mailbox.current_schedule_hints();
  assert!(!user_hints.has_system_messages);
  assert!(user_hints.has_user_messages);
  assert!(!user_hints.backpressure_active);

  mailbox.suspend();
  let suspended_hints = mailbox.current_schedule_hints();
  assert!(!suspended_hints.has_system_messages);
  assert!(!suspended_hints.has_user_messages);
  assert!(!suspended_hints.backpressure_active);
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
