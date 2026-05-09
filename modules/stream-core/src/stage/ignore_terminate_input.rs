#[cfg(test)]
mod tests;

use super::{InHandler, StageContext};
use crate::StreamError;

/// Input-side handler that swallows upstream completion but propagates
/// failures.
///
/// Mirrors Apache Pekko's `pekko.stream.stage.IgnoreTerminateInput`:
/// `on_upstream_finish` is overridden to `Ok(())` (so the enclosing stage
/// ignores completion) while `on_upstream_failure` uses the trait default
/// and re-raises the failure. `on_push` is a no-op acknowledgement.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IgnoreTerminateInput;

impl IgnoreTerminateInput {
  /// Creates a new handler instance.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl<In, Out> InHandler<In, Out> for IgnoreTerminateInput {
  fn on_push(&mut self, _ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_upstream_finish(&mut self, _ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError> {
    Ok(())
  }

  // `on_upstream_failure` uses the trait default: failures are propagated.
}
