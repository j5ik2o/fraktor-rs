#[cfg(test)]
#[path = "materializer_lifecycle_state_test.rs"]
mod tests;

/// Lifecycle state of a materializer.
///
/// Materializers follow a strict lifecycle: `Idle → Running → Stopped`.
/// Once stopped, a materializer cannot be restarted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterializerLifecycleState {
  /// Created but not yet started.
  Idle,
  /// Started and accepting materialization requests.
  Running,
  /// Shut down; no further operations are possible.
  Stopped,
}
