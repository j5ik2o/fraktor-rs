use alloc::string::String;

use super::{SharedKillSwitch, unique_kill_switch::UniqueKillSwitch};
use crate::dsl::{BidiFlow, Flow};

#[cfg(test)]
#[path = "kill_switches_test.rs"]
mod tests;

/// Factory functions for creating kill switches.
pub struct KillSwitches;

impl KillSwitches {
  /// Creates a new shared kill switch.
  #[must_use]
  pub fn shared(name: impl Into<String>) -> SharedKillSwitch {
    SharedKillSwitch::new_named(name)
  }

  /// Creates a new unique kill-switch flow.
  #[must_use]
  pub fn single<T>() -> Flow<T, T, UniqueKillSwitch>
  where
    T: Send + Sync + 'static, {
    UniqueKillSwitch::new().flow()
  }

  /// Creates a bidirectional kill switch backed by identity flows.
  #[must_use]
  pub fn single_bidi<T1, T2>() -> BidiFlow<T1, T1, T2, T2, UniqueKillSwitch>
  where
    T1: Send + Sync + 'static,
    T2: Send + Sync + 'static, {
    UniqueKillSwitch::new().bidi_flow::<T1, T2>()
  }
}
