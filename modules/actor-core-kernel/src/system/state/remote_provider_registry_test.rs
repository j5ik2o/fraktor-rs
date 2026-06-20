use core::any::TypeId;

use super::RemoteProviderRegistry;
use crate::{
  actor::{actor_path::ActorPathScheme, deploy::Deployer, props::DeployableActorFactoryRegistry},
  system::state::AuthorityState,
};

#[test]
fn remote_provider_registry_starts_without_providers_or_authorities() {
  let registry = RemoteProviderRegistry::new(Deployer::default(), DeployableActorFactoryRegistry::new());

  assert!(registry.actor_ref_providers.get(&TypeId::of::<()>()).is_none());
  assert!(registry.actor_ref_provider_callers_by_scheme.get(ActorPathScheme::Fraktor).is_none());
  assert_eq!(registry.remote_authority_registry.state("example"), AuthorityState::Unresolved);
}
