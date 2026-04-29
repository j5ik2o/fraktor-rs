//! Runtime expansion of remote pool routees through the std provider boundary.

use alloc::vec::Vec;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Address,
    actor_path::{ActorPath, ActorPathError},
  },
  routing::{Pool, RemoteRouterConfig, Routee, Router, RouterConfig},
};

use crate::std::provider::{StdRemoteActorRefProvider, remote_routee_expansion_error::RemoteRouteeExpansionError};

/// Expands a remote router configuration into a router with remote actor routees.
pub struct RemoteRouteeExpansion<P, F>
where
  P: Pool,
  F: Fn(usize, &Address) -> Result<ActorPath, ActorPathError>, {
  config:       RemoteRouterConfig<P>,
  path_factory: F,
}

impl<P, F> RemoteRouteeExpansion<P, F>
where
  P: Pool,
  F: Fn(usize, &Address) -> Result<ActorPath, ActorPathError>,
{
  /// Creates a new routee expansion adapter.
  #[must_use]
  pub const fn new(config: RemoteRouterConfig<P>, path_factory: F) -> Self {
    Self { config, path_factory }
  }

  /// Resolves every configured remote routee and returns a router populated with them.
  ///
  /// # Errors
  ///
  /// Returns [`RemoteRouteeExpansionError`] when a routee path cannot be built
  /// or the provider cannot resolve that path into an actor reference.
  pub fn expand(
    &self,
    provider: &mut StdRemoteActorRefProvider,
  ) -> Result<Router<P::Logic>, RemoteRouteeExpansionError> {
    let nodes = self.config.nodes();
    if nodes.is_empty() {
      return Err(RemoteRouteeExpansionError::empty_nodes());
    }

    let mut routees = Vec::with_capacity(self.config.nr_of_instances());
    for index in 0..self.config.nr_of_instances() {
      let node = &nodes[index % nodes.len()];
      let path =
        (self.path_factory)(index, node).map_err(|source| RemoteRouteeExpansionError::routee_path(index, source))?;
      let actor_ref =
        provider.actor_ref(path.clone()).map_err(|source| RemoteRouteeExpansionError::provider(index, path, source))?;
      routees.push(Routee::ActorRef(actor_ref));
    }

    Ok(self.config.create_router().with_routees(routees))
  }
}
