#[cfg(test)]
mod tests;

use super::{OutHandler, StageContext};
use crate::StreamError;

/// Output-side handler that eagerly propagates cancellation events.
///
/// Mirrors Apache Pekko's `pekko.stream.stage.EagerTerminateOutput`:
/// `on_downstream_finish` returns `Ok(())`, which the surrounding stage
/// logic interprets as "propagate the cancellation upstream". `on_pull`
/// is a no-op acknowledgement; actual element production is expected to
/// come from the downstream-facing logic the stage installs next.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EagerTerminateOutput;

impl EagerTerminateOutput {
  /// Creates a new handler instance.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl<In, Out> OutHandler<In, Out> for EagerTerminateOutput {
  fn on_pull(&mut self, _ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError> {
    Ok(())
  }

  // `on_downstream_finish` uses the trait default: cancellation is propagated.
}
