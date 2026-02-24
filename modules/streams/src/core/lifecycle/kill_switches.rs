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

  /// Creates a bidirectional kill switch backed by identity flows.
  #[must_use]
  pub fn single_bidi<T1, T2>() -> crate::core::stage::BidiFlow<T1, T1, T2, T2, UniqueKillSwitch>
  where
    T1: Send + Sync + 'static,
    T2: Send + Sync + 'static, {
    crate::core::stage::BidiFlow::from_flows_mat(
      crate::core::stage::Flow::new(),
      crate::core::stage::Flow::new(),
      UniqueKillSwitch::new(),
    )
  }
}
