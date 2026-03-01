//! Restart DSL facade for flow stages.

#[cfg(test)]
mod tests;

use super::{Flow, RestartSettings};

/// Thin DSL wrapper mirroring Pekko-style `RestartFlow` entry points.
pub struct RestartFlow;

impl RestartFlow {
  /// Applies restart-on-failure backoff settings to a flow.
  #[must_use]
  pub fn with_backoff<In, Out, Mat>(
    flow: Flow<In, Out, Mat>,
    min_backoff_ticks: u32,
    max_restarts: usize,
  ) -> Flow<In, Out, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static, {
    flow.restart_flow_with_backoff(min_backoff_ticks, max_restarts)
  }

  /// Applies restart settings to a flow.
  #[must_use]
  pub fn with_settings<In, Out, Mat>(flow: Flow<In, Out, Mat>, settings: RestartSettings) -> Flow<In, Out, Mat>
  where
    In: Send + Sync + 'static,
    Out: Send + Sync + 'static, {
    flow.restart_flow_with_settings(settings)
  }
}
