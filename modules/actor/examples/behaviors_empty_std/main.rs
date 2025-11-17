//! Demonstrates Behaviors.empty() for waiting state without handling messages.
//!
//! This example shows how to use `Behaviors.empty()` when an actor reaches a state
//! where no more messages are expected, but the actor hasn't stopped yet. Unlike
//! `ignore()`, empty() logs all received messages as unhandled events.

use std::time::Duration;

use fraktor_actor_rs::std::{
  event_stream::{EventStreamEvent, EventStreamSubscriber, EventStreamSubscription},
  typed::{Behavior, Behaviors, TypedActorSystem, TypedProps},
};
use fraktor_utils_rs::core::sync::ArcShared;

#[derive(Debug, Clone)]
enum WorkerCommand {
  DoWork,
  FinishWork,
  StopWorking,
}

/// Worker actor that transitions to empty state after finishing work.
fn worker_behavior() -> Behavior<WorkerCommand> {
  Behaviors::setup(|ctx| {
    println!("[Worker] Started");
    working_behavior(ctx)
  })
}

fn working_behavior(
  _ctx: &mut fraktor_actor_rs::std::typed::actor_prim::TypedActorContext<'_, WorkerCommand>,
) -> Behavior<WorkerCommand> {
  Behaviors::receive_message(|_ctx, message: &WorkerCommand| match message {
    | WorkerCommand::DoWork => {
      println!("[Worker] Doing work...");
      Ok(Behaviors::same())
    },
    | WorkerCommand::FinishWork => {
      println!("[Worker] Finished all work, transitioning to empty state");
      println!("[Worker] Will no longer process messages, but not stopping yet");
      Ok(Behaviors::empty())
    },
    | WorkerCommand::StopWorking => {
      println!("[Worker] Stopping");
      Ok(Behaviors::stopped())
    },
  })
}

/// Event subscriber that logs UnhandledMessage events.
struct UnhandledLogger;

impl EventStreamSubscriber for UnhandledLogger {
  fn on_event(&self, event: &EventStreamEvent) {
    if let EventStreamEvent::UnhandledMessage(unhandled) = event {
      println!(
        "[Event] UnhandledMessage: actor={:?}, message_type={}, timestamp={:?}",
        unhandled.actor(),
        unhandled.message(),
        unhandled.timestamp()
      );
    }
  }
}

fn main() {
  println!("=== Behaviors.empty() Example ===\n");
  println!("This demonstrates an actor that transitions to empty state");
  println!("after completing work, but before stopping.\n");

  // Create typed actor system
  let props = TypedProps::from_behavior_factory(worker_behavior);
  let system = TypedActorSystem::new(&props).expect("Failed to create system");

  // Subscribe to unhandled message events
  let subscriber: ArcShared<dyn EventStreamSubscriber> = ArcShared::new(UnhandledLogger);
  let _subscription: EventStreamSubscription = system.subscribe_event_stream(&subscriber);

  let worker = system.user_guardian_ref();
  let termination = system.when_terminated();

  // Worker is active - message will be processed
  println!("==> Sending DoWork (actor is active)");
  worker.tell(WorkerCommand::DoWork).expect("Failed to send");
  std::thread::sleep(Duration::from_millis(100));

  // Transition to empty state
  println!("\n==> Sending FinishWork (transition to empty)");
  worker.tell(WorkerCommand::FinishWork).expect("Failed to send");
  std::thread::sleep(Duration::from_millis(100));

  // These messages will be treated as unhandled and logged
  println!("\n==> Sending DoWork (actor is empty - will be unhandled)");
  worker.tell(WorkerCommand::DoWork).expect("Failed to send");
  std::thread::sleep(Duration::from_millis(100));

  println!("\n==> Sending another DoWork (actor is empty - will be unhandled)");
  worker.tell(WorkerCommand::DoWork).expect("Failed to send");
  std::thread::sleep(Duration::from_millis(100));

  // Stop the actor
  println!("\n==> Sending StopWorking");
  worker.tell(WorkerCommand::StopWorking).expect("Failed to send");
  std::thread::sleep(Duration::from_millis(100));

  // Terminate system
  println!("\n=== Terminating system ===");
  system.terminate().expect("Failed to terminate");
  while !termination.is_ready() {
    std::thread::yield_now();
  }

  println!("\n=== Example completed ===");
  println!("\nNote: empty() is useful for states like:");
  println!("- Waiting for child actors to stop");
  println!("- Cleanup phase before stopping");
  println!("- Terminal states that should log unexpected messages");
}
