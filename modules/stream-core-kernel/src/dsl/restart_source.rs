//! Restart DSL facade for source stages.

#[cfg(test)]
#[path = "restart_source_test.rs"]
mod tests;

use super::{RestartConfig, source::Source};

/// Thin DSL wrapper mirroring Pekko-style `RestartSource` entry points.
pub struct RestartSource;

impl RestartSource {
  /// Applies restart-on-failure backoff settings to a source.
  #[must_use]
  pub fn with_backoff<Out, Mat>(
    source: Source<Out, Mat>,
    min_backoff_ticks: u32,
    max_restarts: usize,
  ) -> Source<Out, Mat>
  where
    Out: Send + Sync + 'static, {
    source.restart_source_with_backoff(min_backoff_ticks, max_restarts)
  }

  /// Applies restart settings to a source.
  #[must_use]
  pub fn with_settings<Out, Mat>(source: Source<Out, Mat>, settings: RestartConfig) -> Source<Out, Mat>
  where
    Out: Send + Sync + 'static, {
    source.restart_source_with_settings(settings)
  }
}
