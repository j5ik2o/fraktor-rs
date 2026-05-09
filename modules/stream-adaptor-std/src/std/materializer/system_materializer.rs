//! Per-ActorSystem shared materializer extension.

extern crate std;

use std::vec::Vec;

use fraktor_actor_core_rs::actor::extension::Extension;
use fraktor_stream_core_rs::core::{
  materialization::ActorMaterializer,
  snapshot::{MaterializerState, StreamSnapshot},
};

#[cfg(test)]
mod tests;

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

  /// Collects stream snapshots from the underlying materializer.
  #[must_use]
  pub fn stream_snapshots(&self) -> Vec<StreamSnapshot> {
    MaterializerState::stream_snapshots(&self.materializer)
  }
}
