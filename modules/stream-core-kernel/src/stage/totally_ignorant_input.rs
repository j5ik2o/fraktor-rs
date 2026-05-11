#[cfg(test)]
#[path = "totally_ignorant_input_test.rs"]
mod tests;

use super::{InHandler, StageContext};
use crate::StreamError;

/// Input-side handler that absorbs every termination signal.
///
/// Mirrors Apache Pekko's `pekko.stream.stage.TotallyIgnorantInput`:
/// `on_push`, `on_upstream_finish`, and `on_upstream_failure` all return
/// `Ok(())`, so pushes, completions, and failures are silently discarded.
/// Callers are responsible for confining this handler to use cases where
/// absorbing upstream failures cannot mask contract violations.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TotallyIgnorantInput;

impl TotallyIgnorantInput {
  /// Creates a new handler instance.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl<In, Out> InHandler<In, Out> for TotallyIgnorantInput {
  fn on_push(&mut self, _ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_upstream_finish(&mut self, _ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_upstream_failure(
    &mut self,
    _err: StreamError,
    _ctx: &mut dyn StageContext<In, Out>,
  ) -> Result<(), StreamError> {
    Ok(())
  }
}
