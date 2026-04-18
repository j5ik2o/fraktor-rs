#[cfg(test)]
mod tests;

use super::StageContext;
use crate::core::{StreamError, r#impl::CancellationCause};

/// Trait implemented by output-side stage logic handlers.
///
/// Mirrors Apache Pekko's `pekko.stream.stage.OutHandler`. The default
/// implementation for `on_downstream_finish` propagates the cancellation
/// (`Ok(())`); handlers override it to absorb the termination event.
pub trait OutHandler<In, Out> {
  /// Called when downstream has signalled demand for the next element.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] if the handler cannot satisfy the pull request,
  /// for example when the stage has observed an unrecoverable failure or the
  /// upstream has already completed with no buffered elements to emit.
  fn on_pull(&mut self, ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError>;

  /// Called when downstream has cancelled subscription.
  ///
  /// Default implementation propagates cancellation (`Ok(())`).
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] if the handler cannot propagate the cancellation
  /// cause upstream, for example when the upstream port is already terminated
  /// or the stage is in a state that forbids cancellation forwarding.
  fn on_downstream_finish(
    &mut self,
    _cause: CancellationCause,
    _ctx: &mut dyn StageContext<In, Out>,
  ) -> Result<(), StreamError> {
    Ok(())
  }
}
