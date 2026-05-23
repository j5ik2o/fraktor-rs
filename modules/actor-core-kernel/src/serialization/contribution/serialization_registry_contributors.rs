//! Serialization registry contributor collection.

use alloc::vec::Vec;

use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedAccess, SharedLock};

use crate::{
  actor::extension::{Extension, ExtensionId},
  serialization::{
    SerializationExtensionShared,
    contribution::{
      SerializationRegistryContributionError, serialization_registry_contributor::SerializationRegistryContributor,
    },
    serialization_registry::SerializationRegistry,
  },
  system::ActorSystem,
};

struct SerializationRegistryContributorsId;

impl ExtensionId for SerializationRegistryContributorsId {
  type Ext = SerializationRegistryContributors;

  fn create_extension(&self, _system: &ActorSystem) -> Self::Ext {
    SerializationRegistryContributors::new()
  }
}

struct SerializationRegistryContributors {
  contributors: SharedLock<Vec<ArcShared<dyn SerializationRegistryContributor>>>,
}

impl SerializationRegistryContributors {
  fn new() -> Self {
    Self { contributors: SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new()) }
  }

  fn add(&self, contributor: ArcShared<dyn SerializationRegistryContributor>) {
    self.contributors.with_lock(|contributors| contributors.push(contributor));
  }

  fn apply_all(
    &self,
    registry: &ArcShared<SerializationRegistry>,
  ) -> Result<(), SerializationRegistryContributionError> {
    let contributors = self.contributors.with_lock(|contributors| contributors.clone());
    for contributor in contributors {
      contributor.contribute(registry).map_err(SerializationRegistryContributionError::new)?;
    }
    Ok(())
  }
}

impl Extension for SerializationRegistryContributors {}

/// Registers a serialization registry contributor and applies it immediately when possible.
///
/// # Errors
///
/// Returns [`SerializationRegistryContributionError`] when the contributor fails against an
/// already-created serialization registry.
pub fn register_serialization_registry_contributor<C>(
  system: &ActorSystem,
  contributor: C,
) -> Result<(), SerializationRegistryContributionError>
where
  C: SerializationRegistryContributor + 'static, {
  let contributors_id = SerializationRegistryContributorsId;
  let contributor: ArcShared<dyn SerializationRegistryContributor> = ArcShared::new(contributor);
  let contributors = system.extended().register_extension(&contributors_id);
  if let Some(serialization) = system.extended().extension_by_type::<SerializationExtensionShared>() {
    serialization
      .with_read(|extension| contributor.contribute(&extension.registry()))
      .map_err(SerializationRegistryContributionError::new)?;
  }
  contributors.add(contributor);
  Ok(())
}

pub(crate) fn apply_serialization_registry_contributors(
  system: &ActorSystem,
  registry: &ArcShared<SerializationRegistry>,
) -> Result<(), SerializationRegistryContributionError> {
  let contributors_id = SerializationRegistryContributorsId;
  if let Some(contributors) = system.extended().extension(&contributors_id) {
    contributors.apply_all(registry)?;
  }
  Ok(())
}
