//! Key type for identifying timers within a
//! [`TimerSchedulerGeneric`](super::timer_scheduler::TimerSchedulerGeneric).

use alloc::string::String;
use core::fmt;

/// Identifies a named timer managed by a
/// [`TimerSchedulerGeneric`](super::timer_scheduler::TimerSchedulerGeneric).
///
/// Starting a new timer with the same key cancels the previous one.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimerKey {
  name: String,
}

impl TimerKey {
  /// Creates a timer key from the provided name.
  #[must_use]
  pub fn new(name: impl Into<String>) -> Self {
    Self { name: name.into() }
  }

  /// Returns the key name.
  #[must_use]
  pub fn name(&self) -> &str {
    &self.name
  }
}

impl fmt::Debug for TimerKey {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("TimerKey").field(&self.name).finish()
  }
}

impl fmt::Display for TimerKey {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.name)
  }
}
