use fraktor_actor_rs::core::system::ActorSystemGeneric;
use fraktor_utils_rs::core::{runtime_toolbox::NoStdToolbox, sync::ArcShared};

use crate::core::{
  ClusterExtensionConfig, ClusterExtensionId, ClusterProvider, ClusterProviderError, ClusterPubSub, Gossiper,
  IdentityLookup, IdentitySetupError, ActivatedKind,
};

struct StubProvider;
impl ClusterProvider for StubProvider {
  fn start_member(&self) -> Result<(), ClusterProviderError> { Ok(()) }
  fn start_client(&self) -> Result<(), ClusterProviderError> { Ok(()) }
  fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> { Ok(()) }
}

struct StubGossiper;
impl Gossiper for StubGossiper {
  fn start(&self) -> Result<(), &'static str> { Ok(()) }
  fn stop(&self) -> Result<(), &'static str> { Ok(()) }
}

struct StubPubSub;
impl ClusterPubSub for StubPubSub {
  fn start(&self) -> Result<(), crate::core::pub_sub_error::PubSubError> { Ok(()) }
  fn stop(&self) -> Result<(), crate::core::pub_sub_error::PubSubError> { Ok(()) }
}

struct StubIdentity;
impl IdentityLookup for StubIdentity {
  fn setup_member(&self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> { Ok(()) }
  fn setup_client(&self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> { Ok(()) }
}

struct StubBlockList;
impl fraktor_remote_rs::core::BlockListProvider for StubBlockList {
  fn blocked_members(&self) -> Vec<String> { Vec::new() }
}

#[test]
fn registers_extension_and_starts_member() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  let event_stream = system.event_stream();
  // precondition: event stream is usable (smoke)
  assert_eq!(event_stream.subscribers.lock().len(), 0);

  let ext_id = ClusterExtensionId::<NoStdToolbox>::new(
    ClusterExtensionConfig::new().with_advertised_address("fraktor://demo"),
    ArcShared::new(StubProvider),
    ArcShared::new(StubBlockList),
    ArcShared::new(StubGossiper),
    ArcShared::new(StubPubSub),
    ArcShared::new(StubIdentity),
  );

  let ext_shared = system.extended().register_extension(&ext_id);
  let mut ext = ArcShared::clone(&ext_shared);
  let result = ArcShared::make_mut(&mut ext).start_member();
  assert!(result.is_ok());
}
