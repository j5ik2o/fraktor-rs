//! Demonstrates Behaviors.unhandled() for partial message handling.
//!
//! This example shows how to use `Behaviors.unhandled()` to indicate that
//! a message was not handled by the current behavior. Unlike `ignore()`,
//! this emits an `UnhandledMessage` event to the event stream for monitoring.

#[path = "../std_tick_driver_support.rs"]
mod std_tick_driver_support;

use std::time::Duration;

use fraktor_actor_rs::std::{
  event_stream::{
    EventStreamEvent, EventStreamSubscriber, EventStreamSubscriberShared, EventStreamSubscription, subscriber_handle,
  },
  typed::{Behavior, BehaviorSignal, Behaviors, TypedActorSystem, TypedProps},
};

#[derive(Debug, Clone)]
enum Command {
  Ping,
  Pong,
  Unknown,
}

/// Actor that only handles Ping messages, returns unhandled for others.
fn selective_behavior() -> Behavior<Command> {
  Behaviors::receive_message(|_ctx, message: &Command| match message {
    | Command::Ping => {
      println!("Received Ping, responding with message handling");
      Ok(Behaviors::same())
    },
    | _ => {
      println!("Received {:?}, returning unhandled", message);
      Ok(Behaviors::unhandled())
    },
  })
  .receive_signal(|_ctx, signal| {
    if matches!(signal, BehaviorSignal::Started) {
      println!("Actor started");
    }
    Ok(Behaviors::same())
  })
}

/// Simple event subscriber that prints UnhandledMessage events.
struct UnhandledMessageLogger;

impl EventStreamSubscriber for UnhandledMessageLogger {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::UnhandledMessage(unhandled) = event {
      println!(
        "UnhandledMessage event: actor={:?}, message_type={}, timestamp={:?}",
        unhandled.actor(),
        unhandled.message(),
        unhandled.timestamp()
      );
    }
  }
}

fn main() {
  println!("=== Behaviors.unhandled() Example ===\n");

  // Create typed actor system
  let props = TypedProps::from_behavior_factory(selective_behavior);
  let (tick_driver, _pulse_handle) = std_tick_driver_support::hardware_tick_driver_config();
  let system = TypedActorSystem::new(&props, tick_driver).expect("Failed to create system");

  // Subscribe to unhandled message events
  let subscriber: EventStreamSubscriberShared = subscriber_handle(UnhandledMessageLogger);
  let _subscription: EventStreamSubscription = system.subscribe_event_stream(&subscriber);

  let actor_ref = system.user_guardian_ref();
  let termination = system.when_terminated();

  // Send Ping - will be handled
  println!("Sending Ping...");
  actor_ref.tell(Command::Ping).expect("Failed to send Ping");
  std::thread::sleep(Duration::from_millis(100));

  // Send Pong - will be unhandled
  println!("\nSending Pong...");
  actor_ref.tell(Command::Pong).expect("Failed to send Pong");
  std::thread::sleep(Duration::from_millis(100));

  // Send Unknown - will be unhandled
  println!("\nSending Unknown...");
  actor_ref.tell(Command::Unknown).expect("Failed to send Unknown");
  std::thread::sleep(Duration::from_millis(100));

  // Terminate system
  println!("\n=== Terminating system ===");
  system.terminate().expect("Failed to terminate");
  while !termination.is_ready() {
    std::thread::yield_now();
  }

  println!("\n=== Example completed ===");
}
