use super::{Materialized, Materializer, StreamError, StreamPlan};

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
