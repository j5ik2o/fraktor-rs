use cellactor_actor_core_rs::{EventStreamEvent, EventStreamSubscriber};

pub struct LifecyclePrinter;

impl Default for LifecyclePrinter {
  fn default() -> Self {
    Self
  }
}

impl EventStreamSubscriber for LifecyclePrinter {
  fn on_event(&self, event: &EventStreamEvent) {
    if let EventStreamEvent::Lifecycle(lifecycle) = event {
      println!("[lifecycle] name={} pid={} stage={:?}", lifecycle.name(), lifecycle.pid(), lifecycle.stage());
    }
  }
}
