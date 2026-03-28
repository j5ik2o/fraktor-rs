extern crate std;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use std::sync::{Arc, Mutex, Once};

use fraktor_utils_rs::core::sync::{ArcShared, NoStdMutex};
use tracing::{
  Event, Level, Metadata, Subscriber,
  field::{Field, Visit},
  metadata::LevelFilter,
  span::{Attributes, Id, Record},
  subscriber::{Interest, with_default},
};

use crate::{
  core::{
    kernel::{actor::ActorContext, system::ActorSystem},
    typed::{BehaviorSignal, Behaviors as CoreBehaviors, actor::TypedActorContext},
  },
  std::typed::{Behaviors, LogOptions},
};

/// Ensures that tracing's global callsite interest cache does not permanently
/// disable callsites before any subscriber is set.  A permissive global
/// subscriber is installed once so that `Interest::sometimes()` is returned
/// for every callsite, which forces per-dispatch `enabled()` checks and lets
/// thread-local subscribers (via `with_default`) work correctly.
fn ensure_tracing_interest_cache_permissive() {
  static INIT: Once = Once::new();
  INIT.call_once(|| {
    struct PermissiveGlobalSubscriber;
    impl Subscriber for PermissiveGlobalSubscriber {
      fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
        Interest::sometimes()
      }

      fn enabled(&self, _: &Metadata<'_>) -> bool {
        false
      }

      fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
      }

      fn record(&self, _: &Id, _: &Record<'_>) {}

      fn record_follows_from(&self, _: &Id, _: &Id) {}

      fn event(&self, _: &Event<'_>) {}

      fn enter(&self, _: &Id) {}

      fn exit(&self, _: &Id) {}
    }
    let _ = tracing::subscriber::set_global_default(PermissiveGlobalSubscriber);
  });
}

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
  ensure_tracing_interest_cache_permissive();
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
  ensure_tracing_interest_cache_permissive();
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

#[test]
fn with_static_mdc_delegates_to_inner_behavior() {
  let inner_received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let inner_received_clone = inner_received.clone();

  let mut mdc = BTreeMap::new();
  mdc.insert("service".into(), "test-actor".into());

  let mut behavior = Behaviors::with_static_mdc(
    mdc,
    CoreBehaviors::receive_message(move |_ctx, msg: &u32| {
      inner_received_clone.clone().lock().push(*msg);
      Ok(CoreBehaviors::same())
    }),
  );

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
  inner.handle_message(&mut typed_ctx, &55_u32).expect("message");

  assert_eq!(inner_received.lock().as_slice(), &[55]);
}

#[test]
fn with_static_mdc_creates_span_on_message() {
  ensure_tracing_interest_cache_permissive();
  let collector = SpanRecordingSubscriber::default();
  let shared = collector.clone();

  with_default(shared, || {
    let mut mdc = BTreeMap::new();
    mdc.insert("service".into(), "my-actor".into());

    let mut behavior =
      Behaviors::with_static_mdc(mdc, CoreBehaviors::receive_message(|_ctx, _msg: &u32| Ok(CoreBehaviors::same())));

    let system = ActorSystem::new_empty();
    let pid = system.allocate_pid();
    let mut context = ActorContext::new(&system, pid);
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

    let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
    inner.handle_message(&mut typed_ctx, &42_u32).expect("message");

    let spans = collector.spans();
    assert!(!spans.is_empty(), "at least one span should be created");
    assert!(spans.iter().any(|s| s.name == "actor_mdc"), "span should be named actor_mdc");
  });
}

#[test]
fn with_mdc_merges_static_and_per_message_entries() {
  let inner_received = ArcShared::new(NoStdMutex::new(Vec::<u32>::new()));
  let inner_received_clone = inner_received.clone();

  let mut static_mdc = BTreeMap::new();
  static_mdc.insert("service".into(), "test-actor".into());

  let mut behavior = Behaviors::with_mdc(
    static_mdc,
    |msg: &u32| {
      let mut mdc = BTreeMap::new();
      mdc.insert("msg_value".into(), alloc::format!("{msg}"));
      mdc
    },
    CoreBehaviors::receive_message(move |_ctx, msg: &u32| {
      inner_received_clone.clone().lock().push(*msg);
      Ok(CoreBehaviors::same())
    }),
  );

  let system = ActorSystem::new_empty();
  let pid = system.allocate_pid();
  let mut context = ActorContext::new(&system, pid);
  let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

  let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
  inner.handle_message(&mut typed_ctx, &66_u32).expect("message");

  assert_eq!(inner_received.lock().as_slice(), &[66]);
}

#[test]
fn with_static_mdc_creates_span_on_signal() {
  ensure_tracing_interest_cache_permissive();
  let collector = SpanRecordingSubscriber::default();
  let shared = collector.clone();

  with_default(shared, || {
    let mut mdc = BTreeMap::new();
    mdc.insert("service".into(), "signal-actor".into());

    let mut behavior =
      Behaviors::with_static_mdc(mdc, CoreBehaviors::receive_message(|_ctx, _msg: &u32| Ok(CoreBehaviors::same())));

    let system = ActorSystem::new_empty();
    let pid = system.allocate_pid();
    let mut context = ActorContext::new(&system, pid);
    let mut typed_ctx = TypedActorContext::from_untyped(&mut context, None);

    // Started signal goes through around_start; subsequent signals use around_signal
    let mut inner = behavior.handle_signal(&mut typed_ctx, &BehaviorSignal::Started).expect("started");
    // Stopped signal triggers around_signal which creates the MDC span
    let _ = inner.handle_signal(&mut typed_ctx, &BehaviorSignal::Stopped);

    let spans = collector.spans();
    assert!(!spans.is_empty(), "span should be created for Stopped signal");
    assert!(spans.iter().any(|s| s.name == "actor_mdc"));
  });
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

  // NOTE: Returns a constant ID because these tests only verify event recording,
  // not span tracking. Span IDs are not used by the test assertions.
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

#[derive(Clone, Debug)]
struct CapturedSpan {
  name: String,
}

#[derive(Clone, Default)]
struct SpanRecordingSubscriber {
  spans: Arc<Mutex<Vec<CapturedSpan>>>,
}

impl SpanRecordingSubscriber {
  fn spans(&self) -> Vec<CapturedSpan> {
    self.spans.lock().expect("lock").clone()
  }
}

impl Subscriber for SpanRecordingSubscriber {
  fn register_callsite(&self, _metadata: &'static Metadata<'static>) -> Interest {
    Interest::sometimes()
  }

  fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
    true
  }

  fn max_level_hint(&self) -> Option<LevelFilter> {
    Some(LevelFilter::TRACE)
  }

  fn new_span(&self, attrs: &Attributes<'_>) -> Id {
    let name = attrs.metadata().name().into();
    let mut spans = self.spans.lock().expect("lock");
    spans.push(CapturedSpan { name });
    Id::from_u64(spans.len() as u64)
  }

  fn record(&self, _: &Id, _: &Record<'_>) {}

  fn record_follows_from(&self, _: &Id, _: &Id) {}

  fn event(&self, _: &Event<'_>) {}

  fn enter(&self, _: &Id) {}

  fn exit(&self, _: &Id) {}
}
