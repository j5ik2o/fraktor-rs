use core::ops::{Deref, DerefMut};

use crate::{
  core::{
    actor::{ActorContext as CoreActorContext, ChildRef},
    spawn::SpawnError,
  },
  std::{props::Props, system::ActorSystem},
};

/// Context handle for the standard runtime.
pub struct ActorContext<'ctx, 'inner> {
  inner: &'ctx mut CoreActorContext<'inner>,
}

impl<'ctx, 'inner> ActorContext<'ctx, 'inner> {
  /// Builds a std-facing context wrapper from the core context.
  pub const fn new(inner: &'ctx mut CoreActorContext<'inner>) -> Self {
    Self { inner }
  }

  /// Reinterprets a mutable core context reference as the std wrapper without allocation.
  #[must_use]
  pub const fn from_core_mut(core: &'ctx mut CoreActorContext<'inner>) -> Self {
    Self::new(core)
  }

  /// Borrows the underlying core context.
  #[must_use]
  pub const fn as_core(&self) -> &CoreActorContext<'inner> {
    self.inner
  }

  /// Mutably borrows the underlying core context.
  #[must_use]
  pub const fn as_core_mut(&mut self) -> &mut CoreActorContext<'inner> {
    self.inner
  }

  /// Returns the actor system handle.
  #[must_use]
  pub fn system(&self) -> ActorSystem {
    ActorSystem::from_core(self.inner.system().clone())
  }

  /// Requests the actor system to spawn a child actor.
  ///
  /// # Errors
  ///
  /// Returns an error when spawning the child fails.
  pub fn spawn_child(&self, props: &Props) -> Result<ChildRef, SpawnError> {
    self.inner.spawn_child(props.as_core())
  }
}

impl<'inner> Deref for ActorContext<'_, 'inner> {
  type Target = CoreActorContext<'inner>;

  fn deref(&self) -> &Self::Target {
    self.inner
  }
}

impl DerefMut for ActorContext<'_, '_> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.inner
  }
}
