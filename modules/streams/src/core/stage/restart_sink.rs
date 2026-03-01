//! Restart DSL facade for sink stages.

#[cfg(test)]
mod tests;

use super::{RestartSettings, Sink};

/// Thin DSL wrapper mirroring Pekko-style `RestartSink` entry points.
pub struct RestartSink;

impl RestartSink {
  /// Applies restart-on-failure backoff settings to a sink.
  #[must_use]
  pub fn with_backoff<In, Mat>(sink: Sink<In, Mat>, min_backoff_ticks: u32, max_restarts: usize) -> Sink<In, Mat>
  where
    In: Send + Sync + 'static, {
    sink.restart_sink_with_backoff(min_backoff_ticks, max_restarts)
  }

  /// Applies restart settings to a sink.
  #[must_use]
  pub fn with_settings<In, Mat>(sink: Sink<In, Mat>, settings: RestartSettings) -> Sink<In, Mat>
  where
    In: Send + Sync + 'static, {
    sink.restart_sink_with_settings(settings)
  }
}
