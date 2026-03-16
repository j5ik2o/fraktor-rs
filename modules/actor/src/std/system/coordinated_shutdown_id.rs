//! Extension identifier for the coordinated shutdown subsystem.

extern crate std;

use super::coordinated_shutdown::CoordinatedShutdown;
use crate::core::{extension::ExtensionId, system::ActorSystem};

/// Identifier used to register the coordinated shutdown extension.
pub struct CoordinatedShutdownId;

impl ExtensionId for CoordinatedShutdownId {
  type Ext = CoordinatedShutdown;

  /// Creates the coordinated shutdown extension with default phases.
  ///
  /// # Panics
  ///
  /// Panics if the default phase graph contains a cycle. This should never
  /// happen with the built-in phase definitions.
  fn create_extension(&self, _system: &ActorSystem) -> Self::Ext {
    match CoordinatedShutdown::with_default_phases() {
      | Ok(cs) => cs,
      | Err(error) => {
        panic!("default phase graph must not contain cycles: {error}")
      },
    }
  }
}
