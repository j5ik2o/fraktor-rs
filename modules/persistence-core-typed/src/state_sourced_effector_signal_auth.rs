//! Authentication marker for state-sourced effector signals.

#[cfg(test)]
#[path = "state_sourced_effector_signal_auth_test.rs"]
mod tests;

/// Marker that prevents external crates from forging state-sourced effector signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateSourcedEffectorSignalAuth(());

impl StateSourcedEffectorSignalAuth {
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self(())
  }
}
