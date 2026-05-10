#[cfg(test)]
mod tests;

use super::StageActorEnvelope;
use crate::StreamError;

/// Callback contract for messages delivered through a stage actor.
pub trait StageActorReceive: Send {
  /// Handles one stage actor envelope.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the stage actor callback fails.
  fn receive(&mut self, envelope: StageActorEnvelope) -> Result<(), StreamError>;
}
