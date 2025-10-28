use core::ops::{Deref, DerefMut};

/// Handle that wraps a guard object.
#[derive(Debug)]
pub struct GuardHandle<G> {
  guard: G,
}

impl<G> GuardHandle<G> {
  /// Creates a new `GuardHandle`.
  #[must_use]
  pub const fn new(guard: G) -> Self {
    Self { guard }
  }

  /// Extracts the guard object.
  pub fn into_inner(self) -> G {
    self.guard
  }
}

impl<G> Deref for GuardHandle<G> {
  type Target = G;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<G> DerefMut for GuardHandle<G>
where
  G: DerefMut,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}
