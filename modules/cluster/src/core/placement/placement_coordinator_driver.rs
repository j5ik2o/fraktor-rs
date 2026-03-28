//! Driver for placement coordination that emits commands and publishes events.

use alloc::string::String;

use fraktor_actor_rs::core::kernel::{
  event::stream::{EventStreamEvent, EventStreamShared},
  messaging::AnyMessage,
};
use fraktor_utils_rs::core::sync::SharedAccess;

use super::{PlacementCommandResult, PlacementCoordinatorOutcome, PlacementCoordinatorShared};
use crate::core::{grain::GrainKey, identity::LookupError};

/// Driver that orchestrates placement commands.
pub struct PlacementCoordinatorDriver {
  coordinator:  PlacementCoordinatorShared,
  event_stream: EventStreamShared,
}

impl PlacementCoordinatorDriver {
  /// Creates a new driver.
  #[must_use]
  pub const fn new(coordinator: PlacementCoordinatorShared, event_stream: EventStreamShared) -> Self {
    Self { coordinator, event_stream }
  }

  /// Resolves placement and emits required commands.
  ///
  /// # Errors
  ///
  /// Returns an error when placement cannot be resolved.
  pub fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementCoordinatorOutcome, LookupError> {
    let outcome = self.coordinator.with_write(|coordinator| coordinator.resolve(key, now))?;
    self.publish_events();
    Ok(outcome)
  }

  /// Applies a command result and emits follow-up commands.
  ///
  /// # Errors
  ///
  /// Returns an error when placement cannot be resolved.
  pub fn handle_command_result(
    &mut self,
    result: PlacementCommandResult,
  ) -> Result<PlacementCoordinatorOutcome, LookupError> {
    let outcome = self
      .coordinator
      .with_write(|coordinator| coordinator.handle_command_result(result))
      .map_err(|_| LookupError::Pending)?;
    self.publish_events();
    Ok(outcome)
  }

  fn publish_events(&self) {
    let events = self.coordinator.with_write(|coordinator| coordinator.drain_events());
    for event in events {
      let payload = AnyMessage::new(event);
      let extension_event = EventStreamEvent::Extension { name: String::from("cluster"), payload };
      self.event_stream.publish(&extension_event);
    }
  }
}
