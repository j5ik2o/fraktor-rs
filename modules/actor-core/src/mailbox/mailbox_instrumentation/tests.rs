use alloc::vec::Vec;

use cellactor_utils_core_rs::sync::{ArcShared, NoStdMutex};

use super::MailboxInstrumentation;
use crate::{actor_prim::Pid, event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber}, system::SystemState};

#[test]
fn mailbox_instrumentation_new() {
  let system_state = ArcShared::new(SystemState::new());
  let pid = Pid::new(1, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(100), Some(50), Some(80));
  let _ = instrumentation;
}

#[test]
fn mailbox_instrumentation_clone() {
  let system_state = ArcShared::new(SystemState::new());
  let pid = Pid::new(2, 0);
  let instrumentation1 = MailboxInstrumentation::new(system_state.clone(), pid, None, None, None);
  let instrumentation2 = instrumentation1.clone();
  let _ = instrumentation1;
  let _ = instrumentation2;
}

#[test]
fn mailbox_instrumentation_publish() {
  let system_state = ArcShared::new(SystemState::new());
  let pid = Pid::new(3, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(100), Some(50), None);
  instrumentation.publish(10, 5);
}

#[test]
fn mailbox_instrumentation_publish_with_warning() {
  let system_state = ArcShared::new(SystemState::new());
  let pid = Pid::new(4, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(100), Some(50), Some(80));
  instrumentation.publish(80, 5);
  instrumentation.publish(100, 5);
}

#[test]
fn mailbox_instrumentation_emits_pressure_event() {
  let system_state = ArcShared::new(SystemState::new());
  let pid = Pid::new(5, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(4), None, None);

  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber: ArcShared<dyn EventStreamSubscriber> = ArcShared::new(TestSubscriber::new(events.clone()));
  let _subscription = EventStreamGeneric::subscribe_arc(&system_state.event_stream(), &subscriber);

  instrumentation.publish(3, 0);

  assert!(events.lock().iter().any(|event| matches!(event, EventStreamEvent::MailboxPressure(_))));
}

struct TestSubscriber {
  events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>,
}

impl TestSubscriber {
  fn new(events: ArcShared<NoStdMutex<Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for TestSubscriber {
  fn on_event(&self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}
