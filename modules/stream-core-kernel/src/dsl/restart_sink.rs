//! Restart DSL facade for sink stages.

#[cfg(test)]
#[path = "restart_sink_test.rs"]
mod tests;

use super::{RestartConfig, sink::Sink};

/// Thin DSL wrapper mirroring Pekko-style `RestartSink` entry points.
pub struct RestartSink;

impl RestartSink {
  /// Applies restart-on-failure backoff configuration to a sink.
  #[must_use]
  pub fn with_backoff<In, Mat>(sink: Sink<In, Mat>, min_backoff_ticks: u32, max_restarts: usize) -> Sink<In, Mat>
  where
    In: Send + Sync + 'static, {
    sink.restart_sink_with_backoff(min_backoff_ticks, max_restarts)
  }

  /// Applies restart configuration to a sink.
  #[must_use]
  pub fn with_config<In, Mat>(sink: Sink<In, Mat>, config: RestartConfig) -> Sink<In, Mat>
  where
    In: Send + Sync + 'static, {
    sink.restart_sink_with_config(config)
  }
}
