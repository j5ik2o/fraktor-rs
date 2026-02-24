#[cfg(test)]
mod tests;

/// Throttle behavior mode controlling how excess upstream demand is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleMode {
  /// Makes pauses before emitting messages to meet the throttle rate.
  Shaping,
  /// Fails with an error when upstream is faster than the throttle rate.
  Enforcing,
}
