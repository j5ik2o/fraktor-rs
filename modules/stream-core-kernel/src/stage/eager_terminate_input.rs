#[cfg(test)]
#[path = "eager_terminate_input_test.rs"]
mod tests;

use super::{InHandler, StageContext};
use crate::StreamError;

/// Input-side handler that eagerly propagates termination events.
///
/// Mirrors Apache Pekko's `pekko.stream.stage.EagerTerminateInput`:
/// completion is propagated as `Ok(())` and upstream failures are re-raised.
/// `on_push` is a no-op and simply acknowledges the demand by returning
/// `Ok(())`, delegating element consumption to any subsequent handler the
/// stage may install.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EagerTerminateInput;

impl EagerTerminateInput {
  /// Creates a new handler instance.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl<In, Out> InHandler<In, Out> for EagerTerminateInput {
  fn on_push(&mut self, _ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError> {
    Ok(())
  }

  // `on_upstream_finish` / `on_upstream_failure` use the trait defaults:
  // completion and failures are both propagated verbatim.
}
