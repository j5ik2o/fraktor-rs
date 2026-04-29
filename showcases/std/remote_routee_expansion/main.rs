//! Remote routee expansion through the std remote provider.
//!
//! Run with: `cargo run -p fraktor-showcases-std --features advanced --example
//! remote_routee_expansion`

use fraktor_actor_adaptor_std_rs::std::tick_driver::StdTickDriver;
use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Address as ActorAddress, Pid,
    actor_path::{ActorPath, ActorPathError, ActorPathParser},
    actor_ref_provider::{ActorRefProviderHandleShared, LocalActorRefProvider},
    setup::ActorSystemConfig,
  },
  routing::{RemoteRouterConfig, RoundRobinPool, Routee},
  serialization::ActorRefResolveCache,
  system::ActorSystem,
};
use fraktor_remote_adaptor_std_rs::std::{
  provider::{RemoteRouteeExpansion, StdRemoteActorRefProvider},
  tcp_transport::TcpRemoteTransport,
};
use fraktor_remote_core_rs::core::{
  address::{Address as RemoteAddress, RemoteNodeId, UniqueAddress},
  extension::EventPublisher,
  provider::{ProviderError, RemoteActorRef, RemoteActorRefProvider},
};
use fraktor_utils_core_rs::core::sync::{DefaultMutex, SharedLock};

struct ShowcaseRemoteProvider;

impl RemoteActorRefProvider for ShowcaseRemoteProvider {
  fn actor_ref(&mut self, path: ActorPath) -> Result<RemoteActorRef, ProviderError> {
    let node = RemoteNodeId::new("routee-node", "10.0.0.10", Some(2552), 1);
    Ok(RemoteActorRef::new(path, node))
  }

  fn watch(&mut self, _watchee: ActorPath, _watcher: Pid) -> Result<(), ProviderError> {
    Ok(())
  }

  fn unwatch(&mut self, _watchee: ActorPath, _watcher: Pid) -> Result<(), ProviderError> {
    Ok(())
  }
}

fn routee_path(index: usize, node: &ActorAddress) -> Result<ActorPath, ActorPathError> {
  ActorPathParser::parse(&format!("{}/user/worker-{index}", node.to_uri_string()))
}

fn main() {
  let actor_system =
    ActorSystem::new_started_from_config(ActorSystemConfig::new(StdTickDriver::default())).expect("actor system");
  let local_provider = ActorRefProviderHandleShared::new(LocalActorRefProvider::new());
  let transport = SharedLock::new_with_driver::<DefaultMutex<_>>(TcpRemoteTransport::new("127.0.0.1:0", Vec::new()));
  let mut provider = StdRemoteActorRefProvider::new(
    UniqueAddress::new(RemoteAddress::new("local", "127.0.0.1", 2551), 1),
    local_provider,
    Box::new(ShowcaseRemoteProvider),
    transport,
    ActorRefResolveCache::default(),
    EventPublisher::new(actor_system.downgrade()),
  );
  let router_config =
    RemoteRouterConfig::new(RoundRobinPool::new(2), vec![ActorAddress::remote("routee-node", "10.0.0.10", 2552)]);
  let router =
    RemoteRouteeExpansion::new(router_config, routee_path).expand(&mut provider).expect("remote routees should expand");

  assert_eq!(router.routees().len(), 2);
  assert!(matches!(router.routees()[0], Routee::ActorRef(_)));
  actor_system.terminate().expect("terminate actor system");
}
