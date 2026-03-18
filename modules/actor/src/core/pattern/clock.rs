//! Clock trait for abstracting time access in no_std environments.

use core::time::Duration;

/// Abstraction over a monotonic clock.
///
/// Implementations provide the current instant and the ability to compute
/// elapsed time.  This allows the circuit breaker state machine to live in
/// `core/` without depending on `std::time::Instant`.
pub trait Clock: Send + Sync {
  /// The instant type returned by this clock.
  type Instant: Copy + Ord + Send + Sync;

  /// Returns the current instant.
  fn now(&self) -> Self::Instant;

  /// Returns the duration elapsed since `earlier`.
  fn elapsed_since(&self, earlier: Self::Instant) -> Duration;
}
