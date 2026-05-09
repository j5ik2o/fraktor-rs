#[cfg(test)]
mod tests;

use super::{OutHandler, StageContext};
use crate::{StreamError, stage::CancellationCause};

/// Output-side handler that swallows downstream cancellation.
///
/// Mirrors Apache Pekko's `pekko.stream.stage.IgnoreTerminateOutput`:
/// `on_downstream_finish` is overridden to `Ok(())` so the enclosing
/// stage ignores the cancellation request instead of propagating it.
/// `on_pull` is a no-op acknowledgement.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IgnoreTerminateOutput;

impl IgnoreTerminateOutput {
  /// Creates a new handler instance.
  #[must_use]
  pub const fn new() -> Self {
    Self
  }
}

impl<In, Out> OutHandler<In, Out> for IgnoreTerminateOutput {
  fn on_pull(&mut self, _ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError> {
    Ok(())
  }

  fn on_downstream_finish(
    &mut self,
    _cause: CancellationCause,
    _ctx: &mut dyn StageContext<In, Out>,
  ) -> Result<(), StreamError> {
    Ok(())
  }
}
