//! Demonstrates `DeadLetterLogSubscriber` for logging dead letter events via tracing.
//!
//! A `DeadLetterLogSubscriber` is subscribed to the event stream and logs every
//! dead letter event. Unlike Pekko's actor-based `DeadLetterListener`, this is
//! implemented as a lightweight `EventStreamSubscriber` adapter.

use std::{thread, time::Duration};

#[path = "../std_tick_driver_support.rs"]
mod std_tick_driver_support;

use fraktor_actor_rs::{
  core::{
    actor::{Actor, ActorContext},
    error::ActorError,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
  },
  std::{
    event::stream::{DeadLetterLogSubscriber, EventStreamSubscriberShared, subscriber_handle},
    system::ActorSystem,
  },
};
use fraktor_utils_rs::core::sync::SharedAccess;

struct Start;

struct GuardianActor;

impl Actor for GuardianActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<Start>().is_some() {
      // Spawn a child and immediately stop it.
      let child_props = Props::from_fn(|| ChildActor);
      let child = ctx.spawn_child(&child_props).map_err(|e| ActorError::fatal(alloc::format!("{e:?}")))?;
      let child_ref = child.actor_ref().clone();

      ctx.stop_child(&child).map_err(|e| ActorError::from_send_error(&e))?;

      // Wait briefly for the child to stop, then send a message to the
      // stopped actor — this produces a dead letter.
      thread::sleep(Duration::from_millis(30));
      let _ = child_ref.tell(AnyMessage::new("hello stopped actor"));
    }
    Ok(())
  }
}

struct ChildActor;

impl Actor for ChildActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

extern crate alloc;

fn main() {
  let subscriber = tracing_subscriber::FmtSubscriber::builder().with_max_level(tracing::Level::WARN).finish();
  tracing::subscriber::set_global_default(subscriber).expect("subscriber");

  let props = Props::from_fn(|| GuardianActor);
  let (tick_driver, _pulse_handle) = std_tick_driver_support::hardware_tick_driver_config();
  let system = ActorSystem::new(&props, tick_driver).expect("system");

  // Subscribe the DeadLetterLogSubscriber to the event stream.
  let listener: EventStreamSubscriberShared = subscriber_handle(DeadLetterLogSubscriber::new());
  let _subscription = system.subscribe_event_stream(&listener);

  // Trigger the guardian to spawn-and-stop a child, then send a dead letter.
  system.user_guardian_ref().tell(AnyMessage::new(Start)).expect("start");

  thread::sleep(Duration::from_millis(200));

  tracing::warn!("Check the tracing output above for dead letter warnings.");

  system.terminate().expect("terminate");
  let termination = system.when_terminated();
  while !termination.with_read(|af| af.is_ready()) {
    thread::sleep(Duration::from_millis(10));
  }
}
