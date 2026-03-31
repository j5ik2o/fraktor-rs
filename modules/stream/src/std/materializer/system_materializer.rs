//! Per-ActorSystem shared materializer extension.

extern crate std;

#[cfg(test)]
mod tests;

use fraktor_actor_rs::core::kernel::actor::extension::Extension;

use crate::core::materialization::ActorMaterializer;

/// Per-ActorSystem shared materializer (Pekko `SystemMaterializer` equivalent).
///
/// Registered as an [`Extension`] on the actor system to provide a single
/// shared [`ActorMaterializer`] instance. Use
/// [`SystemMaterializerId`](crate::std::materializer::SystemMaterializerId)
/// to register and retrieve this extension.
pub struct SystemMaterializer {
  materializer: ActorMaterializer,
}

impl Extension for SystemMaterializer {}

impl SystemMaterializer {
  /// Creates a new system materializer wrapping the given materializer.
  #[must_use]
  pub const fn new(materializer: ActorMaterializer) -> Self {
    Self { materializer }
  }

  /// Returns a reference to the underlying materializer.
  #[must_use]
  pub const fn materializer(&self) -> &ActorMaterializer {
    &self.materializer
  }

  /// Returns a mutable reference to the underlying materializer.
  #[must_use]
  pub const fn materializer_mut(&mut self) -> &mut ActorMaterializer {
    &mut self.materializer
  }
}
