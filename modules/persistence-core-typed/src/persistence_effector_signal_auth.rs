//! Authentication marker for persistence effector signals.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistenceEffectorSignalAuth(());

impl PersistenceEffectorSignalAuth {
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self(())
  }
}
