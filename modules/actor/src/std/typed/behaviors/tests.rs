extern crate std;

use alloc::{string::String, vec::Vec};
use std::sync::{Arc, Mutex};

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};
use tracing::{
  Event, Level, Metadata, Subscriber,
  field::{Field, Visit},
  span::{Attributes, Id, Record},
  subscriber::with_default,
};

use crate::{
  core::{
    actor::ActorContext,
    system::ActorSystem,
    typed::{BehaviorSignal, Behaviors as CoreBehaviors, actor::TypedActorContext},
  },
  std::typed::{Behaviors, LogOptions},
};

#[test]
fn log_messages_delegates_to_inner_behavior() {
  let inner_received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let inner_received_clone = inner_received.clone();

  let mut behavior = Behaviors::log_messages(CoreBehaviors::receive_message(move |_ctx, msg: &u32| {
    let received = inner_received_clone.clone();
    received.lock().push(*msg);
    Ok(CoreBehaviors::same())
  }));

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
  inner.handle_message(&mut typed_ctx, &77u32).expect("message");

  let captured = inner_received.lock();
  assert_eq!(captured.len(), 1, "inner behavior should have received the message");
  assert_eq!(captured[0], 77);
}

#[test]
fn log_messages_with_opts_delegates_to_inner_behavior() {
  let inner_received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let inner_received_clone = inner_received.clone();

  let options =
    LogOptions::default().with_enabled(false).with_level(tracing::Level::INFO).with_logger_name("typed.test");
  let mut behavior = Behaviors::log_messages_with_opts(
    options,
    CoreBehaviors::receive_message(move |_ctx, msg: &u32| {
      let received = inner_received_clone.clone();
      received.lock().push(*msg);
      Ok(CoreBehaviors::same())
    }),
  );

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
  inner.handle_message(&mut typed_ctx, &78u32).expect("message");

  assert_eq!(inner_received.lock().as_slice(), &[78]);
}

#[test]
fn log_messages_with_opts_skips_logging_when_disabled() {
  let collector = RecordingSubscriber::default();
  let shared = collector.clone();

  with_default(shared, || {
    let mut behavior = Behaviors::log_messages_with_opts(
      LogOptions::new().with_enabled(false),
      CoreBehaviors::receive_message(|_ctx, _msg: &u32| Ok(CoreBehaviors::same())),
    );

    let system = ActorSystem::new_empty();
    let pid = system.allocate_pid();
    let mut context = ActorContext::new(&system, pid);
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

    let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
    inner.handle_message(&mut typed_ctx, &90_u32).expect("message");
  });

  assert!(collector.events().is_empty());
}

#[test]
fn log_messages_with_opts_records_level_and_logger_name() {
  let collector = RecordingSubscriber::default();
  let shared = collector.clone();

  with_default(shared, || {
    let options = LogOptions::new().with_level(tracing::Level::INFO).with_logger_name("typed.behaviors.test");
    let mut behavior = Behaviors::log_messages_with_opts(
      options,
      CoreBehaviors::receive_message(|_ctx, _msg: &u32| Ok(CoreBehaviors::same())),
    );

    let system = ActorSystem::new_empty();
    let pid = system.allocate_pid();
    let mut context = ActorContext::new(&system, pid);
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

    let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
    inner.handle_message(&mut typed_ctx, &91_u32).expect("message");
  });

  let events = collector.events();
  assert_eq!(events.len(), 1);
  assert_eq!(events[0].level, Level::INFO);
  assert_eq!(events[0].logger_name.as_deref(), Some("typed.behaviors.test"));
}

#[test]
fn receive_message_handles_message() {
  let received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let received_clone = received.clone();
  let captured_pid = ArcShared::new(NoStdMutex::new(0u64));
  let captured_pid_clone = captured_pid.clone();

  let mut behavior = Behaviors::receive_message(move |ctx, msg: &u32| {
    received_clone.lock().push(*msg);
    *captured_pid_clone.lock() = ctx.pid().value();
    Ok(CoreBehaviors::same())
  });

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  behavior.handle_message(&mut typed_ctx, &11u32).expect("message");

  assert_eq!(received.lock().as_slice(), &[11]);
  assert_eq!(*captured_pid.lock(), typed_ctx.pid().value());
}

#[derive(Clone, Debug)]
struct CapturedEvent {
  level:       Level,
  logger_name: Option<String>,
}

#[derive(Clone, Default)]
struct RecordingSubscriber {
  events: Arc<Mutex<Vec<CapturedEvent>>>,
}

impl RecordingSubscriber {
  fn events(&self) -> Vec<CapturedEvent> {
    self.events.lock().expect("lock").clone()
  }
}

impl Subscriber for RecordingSubscriber {
  fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
    true
  }

  fn new_span(&self, _: &Attributes<'_>) -> Id {
    Id::from_u64(0)
  }

  fn record(&self, _: &Id, _: &Record<'_>) {}

  fn record_follows_from(&self, _: &Id, _: &Id) {}

  fn event(&self, event: &Event<'_>) {
    let mut visitor = EventVisitor::default();
    event.record(&mut visitor);
    self
      .events
      .lock()
      .expect("lock")
      .push(CapturedEvent { level: *event.metadata().level(), logger_name: visitor.logger_name });
  }

  fn enter(&self, _: &Id) {}

  fn exit(&self, _: &Id) {}
}

#[derive(Default)]
struct EventVisitor {
  logger_name: Option<String>,
}

impl Visit for EventVisitor {
  fn record_str(&mut self, field: &Field, value: &str) {
    if field.name() == "logger_name" {
      self.logger_name = Some(value.to_owned());
    }
  }

  fn record_debug(&mut self, _field: &Field, _value: &dyn core::fmt::Debug) {}
}
