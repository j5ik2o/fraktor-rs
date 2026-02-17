use super::{SharedKillSwitch, unique_kill_switch::UniqueKillSwitch};

#[cfg(test)]
mod tests;

/// Factory functions for creating kill switches.
pub struct KillSwitches;

impl KillSwitches {
  /// Creates a new shared kill switch.
  #[must_use]
  pub fn shared() -> SharedKillSwitch {
    SharedKillSwitch::new()
  }

  /// Creates a new unique kill switch.
  #[must_use]
  pub fn single() -> UniqueKillSwitch {
    UniqueKillSwitch::new()
  }
}
