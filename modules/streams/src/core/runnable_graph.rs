use super::StreamPlan;

/// Graph ready for materialization.
pub struct RunnableGraph<Mat> {
  plan:         StreamPlan,
  materialized: Mat,
}

impl<Mat> RunnableGraph<Mat> {
  pub(super) const fn new(plan: StreamPlan, materialized: Mat) -> Self {
    Self { plan, materialized }
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
