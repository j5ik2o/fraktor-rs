//! Authentication marker for event-sourced effector signals.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventSourcedEffectorSignalAuth(());

impl EventSourcedEffectorSignalAuth {
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self(())
  }
}
