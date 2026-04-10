extern crate alloc;

use alloc::vec::Vec;
use core::{hint::spin_loop, num::NonZeroUsize};

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::kernel::{
  actor::{
    Actor, ActorContext,
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::{MailboxConfig, Props},
    scheduler::tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  dispatch::mailbox::{Mailbox, MailboxOverflowStrategy, MailboxPolicy, ScheduleHints},
  event::{
    logging::LogLevel,
    stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  },
  system::{ActorSystem, SpinBlocker},
};

struct PassiveActor;

impl Actor for PassiveActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct RecordingSubscriber {
  events: ArcShared<SpinSyncMutex<Vec<EventStreamEvent>>>,
}

impl EventStreamSubscriber for RecordingSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}

#[test]
fn mailbox_metrics_and_warnings_are_emitted() {
  let warn_threshold = NonZeroUsize::new(1).unwrap();
  let capacity = NonZeroUsize::new(2).unwrap();
  let mailbox_config = MailboxConfig::new(MailboxPolicy::bounded(capacity, MailboxOverflowStrategy::DropNewest, None))
    .with_warn_threshold(Some(warn_threshold));
  let props = Props::from_fn(|| PassiveActor).with_mailbox_config(mailbox_config);
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let system = ActorSystem::new(&props, tick_driver).expect("system");

  let events = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let subscriber = subscriber_handle(RecordingSubscriber { events: events.clone() });
  let _subscription = system.subscribe_event_stream(&subscriber);

  system.user_guardian_ref().tell(AnyMessage::new("first"));

  wait_until(|| {
    let events = events.lock().clone();
    let has_metrics = events.iter().any(|event| matches!(event, EventStreamEvent::Mailbox(_)));
    let has_warning =
      events.iter().any(|event| matches!(event, EventStreamEvent::Log(log) if log.level() == LogLevel::Warn));
    has_metrics && has_warning
  });

  system.terminate().expect("terminate");
  system.run_until_terminated(&SpinBlocker);
}

#[test]
fn mailbox_schedule_requests_follow_state_engine() {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));
  let hints = ScheduleHints { has_system_messages: true, has_user_messages: false, backpressure_active: false };

  assert!(mailbox.request_schedule(hints));
  assert!(!mailbox.request_schedule(hints));

  mailbox.set_running();
  assert!(!mailbox.request_schedule(hints));

  assert!(matches!(mailbox.finish_run(), crate::core::kernel::dispatch::mailbox::RunFinishOutcome::Continue {
    pending_reschedule: true,
  }));
  assert!(mailbox.request_schedule(hints));
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
