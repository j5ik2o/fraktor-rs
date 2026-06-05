//! Actor-core manifest preservation rule for cluster messages.

#[cfg(test)]
#[path = "cluster_message_manifest_test.rs"]
mod tests;

use alloc::string::{String, ToString};

/// Opaque actor-core manifest preserved by cluster message serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterMessageManifest {
  actor_manifest: Option<String>,
}

impl ClusterMessageManifest {
  /// Creates a cluster manifest holder from actor-core manifest metadata.
  #[must_use]
  pub fn from_actor_manifest(manifest: Option<&str>) -> Self {
    Self { actor_manifest: manifest.map(ToString::to_string) }
  }

  /// Returns the preserved actor-core manifest.
  #[must_use]
  pub fn actor_manifest(&self) -> Option<&str> {
    self.actor_manifest.as_deref()
  }

  /// Converts this holder back into actor-core manifest metadata.
  #[must_use]
  pub fn into_actor_manifest(self) -> Option<String> {
    self.actor_manifest
  }
}
