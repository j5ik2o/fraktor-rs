//! No-op user guardian actor used by convenience actor-system factories.

#[cfg(test)]
mod tests;

use crate::core::kernel::actor::{Actor, ActorContext, error::ActorError, messaging::AnyMessageView};

/// User guardian actor that accepts every message without side effects.
pub(crate) struct NoopGuardianActor;

impl NoopGuardianActor {
  /// Creates a no-op user guardian actor.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self
  }
}

impl Actor for NoopGuardianActor {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}
