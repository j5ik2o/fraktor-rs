//! Authentication marker for durable state signals.

/// Marker that prevents external crates from forging durable state signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DurableStateSignalAuth(());

impl DurableStateSignalAuth {
  #[allow(dead_code)]
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self(())
  }
}
