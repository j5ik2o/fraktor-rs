use core::{
  fmt::{Debug, Write},
  sync::atomic::{AtomicU64, Ordering},
};
use std::{println, string::String, vec::Vec};

use fraktor_actor_adaptor_std_rs::std::event::logging::TracingLoggerSubscriber;
use fraktor_actor_core_rs::core::kernel::{
  event::{
    logging::LogLevel,
    stream::{EventStreamSubscriberShared, EventStreamSubscription},
  },
  system::ActorSystem,
};
use fraktor_actor_core_typed_rs::TypedActorSystem;
use tracing::{
  Event, Id, Metadata, Subscriber,
  field::{Field, Visit},
  span::{Attributes, Record},
  subscriber::set_global_default,
};

/// Subscribes a `tracing` logger to a kernel actor system.
#[must_use]
pub fn subscribe_kernel_tracing_logger(system: &ActorSystem) -> EventStreamSubscription {
  init_stdout_tracing();
  system.subscribe_event_stream(&tracing_logger_subscriber())
}

/// Subscribes a `tracing` logger to a typed actor system.
#[must_use]
pub fn subscribe_typed_tracing_logger<M>(system: &TypedActorSystem<M>) -> EventStreamSubscription
where
  M: Send + Sync + 'static, {
  init_stdout_tracing();
  system.subscribe_event_stream(&tracing_logger_subscriber())
}

fn tracing_logger_subscriber() -> EventStreamSubscriberShared {
  EventStreamSubscriberShared::new(Box::new(TracingLoggerSubscriber::new(LogLevel::Trace)))
}

fn init_stdout_tracing() -> bool {
  set_global_default(StdoutTracingSubscriber::default()).is_ok()
}

#[derive(Default)]
struct StdoutTracingSubscriber {
  next_span_id: AtomicU64,
}

impl Subscriber for StdoutTracingSubscriber {
  fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
    true
  }

  fn new_span(&self, _span: &Attributes<'_>) -> Id {
    Id::from_u64(self.next_span_id.fetch_add(1, Ordering::Relaxed) + 1)
  }

  fn record(&self, _span: &Id, _values: &Record<'_>) {}

  fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

  fn event(&self, event: &Event<'_>) {
    let mut visitor = TracingEventVisitor::default();
    event.record(&mut visitor);

    let metadata = event.metadata();
    match visitor.message {
      | Some(message) if visitor.fields.is_empty() => {
        println!("[actor-log {} {}] {message}", metadata.level(), metadata.target());
      },
      | Some(message) => {
        println!("[actor-log {} {}] {message} {}", metadata.level(), metadata.target(), visitor.fields.join(" "));
      },
      | None => {
        println!("[actor-log {} {}] {}", metadata.level(), metadata.target(), visitor.fields.join(" "));
      },
    }
  }

  fn enter(&self, _span: &Id) {}

  fn exit(&self, _span: &Id) {}
}

#[derive(Default)]
struct TracingEventVisitor {
  message: Option<String>,
  fields:  Vec<String>,
}

impl Visit for TracingEventVisitor {
  fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
    let mut rendered = String::new();
    write!(&mut rendered, "{value:?}").expect("format tracing field");
    if field.name() == "message" {
      self.message = Some(rendered);
    } else {
      self.fields.push(format!("{}={rendered}", field.name()));
    }
  }
}
