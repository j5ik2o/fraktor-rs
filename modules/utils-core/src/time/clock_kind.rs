//! Declares the classification for time sources.

/// Clock flavor used by the scheduler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockKind {
  /// Deterministic clocks driven manually during tests.
  Deterministic,
  /// Host clocks that rely on std runtime.
  RealtimeHost,
  /// Hardware timers available in embedded targets.
  RealtimeHardware,
}
