//! Restart count policy for supervisor strategies.
//!
//! Pekko equivalent: `maxNrOfRetries: Int` parameter on `OneForOneStrategy`
//! and `AllForOneStrategy`
//! (`references/pekko/actor/src/main/scala/org/apache/pekko/actor/FaultHandling.scala`,
//! and typed counterpart at `actor-typed/.../SupervisorStrategy.scala`).
//!
//! The Pekko contract uses three discriminated values encoded in a single
//! signed integer:
//!
//! | Pekko `maxNrOfRetries` | Meaning                              | `RestartLimit` |
//! |------------------------|--------------------------------------|----------------|
//! | `-1`                   | Unlimited restarts                   | [`RestartLimit::Unlimited`]             |
//! | `0`                    | No retry — stop on first failure     | [`RestartLimit::WithinWindow`]`(0)`     |
//! | `n > 0`                | At most `n` restarts within `window` | [`RestartLimit::WithinWindow`]`(n)`     |
//!
//! This enum replaces the prior `u32` representation in which `0` was used as
//! the "unlimited" sentinel — that encoding inverted Pekko's contract and made
//! porting Pekko configuration values unsafe.

/// Maximum restart count policy applied by a supervisor strategy.
///
/// See the module-level docs for the mapping to Pekko's `maxNrOfRetries`
/// contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartLimit {
  /// Unlimited restarts (Pekko `maxNrOfRetries = -1`).
  Unlimited,
  /// At most the stored number of restarts are allowed within the configured
  /// `within` window (Pekko `maxNrOfRetries = n`, including `n = 0` which
  /// denotes "no retry, immediate stop").
  WithinWindow(u32),
}
