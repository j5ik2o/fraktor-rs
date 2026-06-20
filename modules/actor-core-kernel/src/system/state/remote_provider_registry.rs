//! Remote provider and deployment state owned by SystemState.

#[cfg(test)]
#[path = "remote_provider_registry_test.rs"]
mod tests;

use super::{
  ActorRefProviderCallers, ActorRefProviders, RemoteAuthorityRegistry, RemoteDeploymentHookDynShared,
  RemoteWatchHookDynShared,
};
use crate::actor::{deploy::Deployer, props::DeployableActorFactoryRegistry};

/// Owns provider, remote hook, authority, and deployment state.
pub(crate) struct RemoteProviderRegistry {
  pub(crate) actor_ref_providers: ActorRefProviders,
  pub(crate) actor_ref_provider_callers_by_scheme: ActorRefProviderCallers,
  pub(crate) remote_deployment_hook: RemoteDeploymentHookDynShared,
  pub(crate) remote_watch_hook: RemoteWatchHookDynShared,
  pub(crate) deployer: Deployer,
  pub(crate) deployable_actor_factory_registry: DeployableActorFactoryRegistry,
  pub(crate) remote_authority_registry: RemoteAuthorityRegistry,
}

impl RemoteProviderRegistry {
  pub(crate) fn new(deployer: Deployer, deployable_actor_factory_registry: DeployableActorFactoryRegistry) -> Self {
    Self {
      actor_ref_providers: ActorRefProviders::default(),
      actor_ref_provider_callers_by_scheme: ActorRefProviderCallers::default(),
      remote_deployment_hook: RemoteDeploymentHookDynShared::noop(),
      remote_watch_hook: RemoteWatchHookDynShared::noop(),
      deployer,
      deployable_actor_factory_registry,
      remote_authority_registry: RemoteAuthorityRegistry::default(),
    }
  }
}
