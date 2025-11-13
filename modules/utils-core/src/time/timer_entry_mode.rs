//! Execution modes for timer entries.

/// Execution mode for a scheduled timer entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimerEntryMode {
  /// Single-shot timer.
  OneShot,
  /// Fixed rate execution.
  FixedRate,
  /// Fixed delay execution.
  FixedDelay,
}
