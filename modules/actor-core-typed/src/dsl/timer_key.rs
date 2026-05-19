//! Key type for identifying timers within a
//! [`TimerScheduler`](super::timer_scheduler::TimerScheduler).

use alloc::string::String;
use core::fmt::{Debug, Display, Formatter, Result as FmtResult};

/// Identifies a named timer managed by a
/// [`TimerScheduler`](super::timer_scheduler::TimerScheduler).
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

impl Debug for TimerKey {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    f.debug_tuple("TimerKey").field(&self.name).finish()
  }
}

impl Display for TimerKey {
  fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
    write!(f, "{}", self.name)
  }
}
