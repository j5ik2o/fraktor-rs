use super::{Materialized, Materializer, SharedKillSwitch, StreamError, StreamPlan};

#[cfg(test)]
mod tests;

/// Immutable graph blueprint ready for materialization.
///
/// `RunnableGraph` does not execute by itself.
/// Calling [`Self::run`] hands this blueprint to a materializer, which creates
/// runtime state and starts stream execution.
pub struct RunnableGraph<Mat> {
  plan:         StreamPlan,
  materialized: Mat,
}

impl<Mat> RunnableGraph<Mat> {
  pub(super) const fn new(plan: StreamPlan, materialized: Mat) -> Self {
    Self { plan, materialized }
  }

  /// Binds a pre-created shared kill switch to this graph before materialization.
  #[must_use]
  pub fn with_shared_kill_switch(mut self, shared_kill_switch: &SharedKillSwitch) -> Self {
    self.plan = self.plan.with_shared_kill_switch_state(shared_kill_switch.state_handle());
    self
  }

  /// Materializes the graph with the provided materializer.
  ///
  /// # Errors
  ///
  /// Returns [`StreamError`] when materialization fails.
  pub fn run<M>(self, materializer: &mut M) -> Result<Materialized<Mat, M::Toolbox>, StreamError>
  where
    M: Materializer, {
    materializer.materialize(self)
  }

  /// Returns the materialized value.
  #[must_use]
  pub const fn materialized(&self) -> &Mat {
    &self.materialized
  }

  pub(super) fn into_parts(self) -> (StreamPlan, Mat) {
    (self.plan, self.materialized)
  }
}
