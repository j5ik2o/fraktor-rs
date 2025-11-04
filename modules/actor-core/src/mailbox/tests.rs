extern crate alloc;

use alloc::vec::Vec;
use core::{hint::spin_loop, num::NonZeroUsize};

use cellactor_utils_core_rs::sync::{ArcShared, NoStdMutex};

use crate::{
  NoStdToolbox,
  actor_prim::{Actor, ActorContext},
  error::ActorError,
  eventstream::{EventStreamEvent, EventStreamSubscriber},
  logging::LogLevel,
  mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  messaging::{AnyMessage, AnyMessageView},
  props::{MailboxConfig, Props},
  system::ActorSystem,
};

struct PassiveActor;

impl Actor<NoStdToolbox> for PassiveActor {
  fn receive(
    &mut self,
    _ctx: &mut ActorContext<'_, NoStdToolbox>,
    _message: AnyMessageView<'_, NoStdToolbox>,
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
  let props = Props::<NoStdToolbox>::from_fn(|| PassiveActor).with_mailbox(mailbox_config);
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

fn wait_until(condition: impl Fn() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}
