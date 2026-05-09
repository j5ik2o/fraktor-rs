#[cfg(test)]
mod tests;

use super::StageContext;
use crate::StreamError;

/// Trait implemented by input-side stage logic handlers.
///
/// Mirrors Apache Pekko's `pekko.stream.stage.InHandler`. The default
/// implementations encode Pekko's termination directives: completion is
/// propagated downstream (`Ok(())`) and upstream failures are re-raised
/// (`Err(err)`). Handlers override these methods to absorb or transform
/// termination events.
pub trait InHandler<In, Out> {
  /// Called when an element is available on the input.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] if the handler cannot process the pushed element,
  /// for example when the downstream has already been cancelled or the stage
  /// has observed an unrecoverable failure in a previous tick.
  fn on_push(&mut self, ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError>;

  /// Called when the upstream has completed.
  ///
  /// Default implementation propagates completion (`Ok(())`).
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] if the handler cannot propagate the completion
  /// downstream, for example when the stage is in a state that forbids
  /// completion (such as a buffered element that must be emitted first).
  fn on_upstream_finish(&mut self, _ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError> {
    Ok(())
  }

  /// Called when the upstream has failed.
  ///
  /// Default implementation re-raises the failure.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when the handler either re-raises the upstream
  /// failure (the default behaviour) or cannot recover from it and must
  /// propagate a different error downstream.
  fn on_upstream_failure(&mut self, err: StreamError, _ctx: &mut dyn StageContext<In, Out>) -> Result<(), StreamError> {
    Err(err)
  }
}
