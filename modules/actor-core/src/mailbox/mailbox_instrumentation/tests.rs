use alloc::vec::Vec;

use fraktor_utils_core_rs::core::sync::{ArcShared, NoStdMutex};

use super::MailboxInstrumentation;
use crate::{
  actor_prim::Pid,
  event_stream::{EventStreamEvent, EventStreamGeneric, EventStreamSubscriber},
  mailbox::BackpressurePublisherGeneric,
  system::SystemState,
};

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

#[test]
fn mailbox_instrumentation_notifies_backpressure_publisher() {
  let system_state = ArcShared::new(SystemState::new());
  let pid = Pid::new(6, 0);
  let mut instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(4), None, None);

  let captured = ArcShared::new(NoStdMutex::new(Vec::new()));
  let publisher = BackpressurePublisherGeneric::from_fn({
    let captured = captured.clone();
    move |event: &crate::mailbox::MailboxPressureEvent| {
      captured.lock().push((event.pid(), event.user_len()));
    }
  });
  instrumentation.attach_backpressure_publisher(publisher);

  instrumentation.publish(3, 0);

  let entries = captured.lock();
  assert_eq!(entries.len(), 1);
  assert_eq!(entries[0].0, pid);
  assert_eq!(entries[0].1, 3);
}

#[test]
fn mailbox_pressure_event_captures_threshold() {
  let system_state = ArcShared::new(SystemState::new());
  let pid = Pid::new(7, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(4), None, Some(3));

  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber: ArcShared<dyn EventStreamSubscriber> = ArcShared::new(TestSubscriber::new(events.clone()));
  let _subscription = EventStreamGeneric::subscribe_arc(&system_state.event_stream(), &subscriber);

  instrumentation.publish(3, 0);

  let guard = events.lock();
  let pressure = guard
    .iter()
    .find_map(|event| match event {
      | EventStreamEvent::MailboxPressure(evt) => Some(evt.threshold()),
      | _ => None,
    })
    .flatten();
  assert_eq!(pressure, Some(3));
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
