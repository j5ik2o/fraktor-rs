use alloc::{string::String, vec::Vec};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    actor_ref_provider::{ActorRefProvider, ActorRefProviderHandleShared},
    error::{ActorError, SendError},
    extension::ExtensionInstallers,
    messaging::AnyMessage,
    setup::ActorSystemConfig,
  },
  system::{ActorSystem, TerminationSignal},
};
use fraktor_actor_core_typed_rs::{ExtensionSetup, TypedActorSystem, TypedProps, dsl::Behaviors};
use fraktor_cluster_core_kernel_rs::{
  activation::{
    ActivatedKind, IdentityLookup, IdentitySetupError, LookupError, PlacementDecision, PlacementLocality,
    PlacementResolution,
  },
  cluster_provider::NoopClusterProvider,
  extension::{ClusterExtension, ClusterExtensionConfig, ClusterExtensionInstaller},
  grain::GrainKey,
};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex, shared::Shared};

use crate::{ClusterSharding, ClusterShardingId, ClusterShardingSetup, GrainTypeKey};

#[derive(Debug)]
struct CounterMessage;

struct StaticIdentityLookup {
  authority:        String,
  registered_kinds: ArcShared<SpinSyncMutex<Vec<String>>>,
}

impl StaticIdentityLookup {
  fn new(authority: &str, registered_kinds: ArcShared<SpinSyncMutex<Vec<String>>>) -> Self {
    Self { authority: authority.to_string(), registered_kinds }
  }
}

impl IdentityLookup for StaticIdentityLookup {
  fn setup_member(&mut self, kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    *self.registered_kinds.lock() = kinds.iter().map(|kind| kind.name().to_string()).collect();
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let pid = alloc::format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }
}

fn build_typed_system(registered_kinds: ArcShared<SpinSyncMutex<Vec<String>>>) -> TypedActorSystem<CounterMessage> {
  let registered_for_factory = registered_kinds.clone();
  let cluster_installer =
    ClusterExtensionInstaller::new(ClusterExtensionConfig::new().with_advertised_address("node1:8080"), {
      |_event_stream, _block_list, _address| Box::new(NoopClusterProvider::new())
    })
    .with_identity_lookup_factory(move || {
      Box::new(StaticIdentityLookup::new("node1:8080", registered_for_factory.clone()))
    });
  let installers = ExtensionInstallers::default().with_extension_installer(cluster_installer).with_extension_installer(
    ExtensionSetup::new(ClusterShardingId::new(), |system| {
      ClusterSharding::try_from_system(system).expect("cluster sharding")
    }),
  );
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_extension_installers(installers)
    .with_actor_ref_provider_installer(|system: &ActorSystem| {
      let provider = ActorRefProviderHandleShared::new(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = TypedProps::<CounterMessage>::from_behavior_factory(Behaviors::ignore);
  TypedActorSystem::create_from_props(&props, config).expect("typed system")
}

#[test]
fn cluster_sharding_setup_registers_custom_factory_during_bootstrap() {
  let registered_kinds = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let registered_for_factory = registered_kinds.clone();
  let cluster_installer =
    ClusterExtensionInstaller::new(ClusterExtensionConfig::new().with_advertised_address("node1:8080"), {
      |_event_stream, _block_list, _address| Box::new(NoopClusterProvider::new())
    })
    .with_identity_lookup_factory(move || {
      Box::new(StaticIdentityLookup::new("node1:8080", registered_for_factory.clone()))
    });
  let installers = ExtensionInstallers::default().with_extension_installer(cluster_installer).with_extension_installer(
    ClusterShardingSetup::new(|system| ClusterSharding::try_from_system(system).expect("cluster sharding from setup")),
  );
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_extension_installers(installers)
    .with_actor_ref_provider_installer(|system: &ActorSystem| {
      let provider = ActorRefProviderHandleShared::new(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = TypedProps::<CounterMessage>::from_behavior_factory(Behaviors::ignore);
  let system = TypedActorSystem::create_from_props(&props, config).expect("typed system");
  let extension = system.as_untyped().extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  extension.start_member().expect("start member");

  let sharding =
    system.as_untyped().extended().extension(&ClusterShardingId::new()).expect("cluster sharding installed via setup");
  sharding.with_ref(|sharding| {
    sharding.init_type_key(GrainTypeKey::<CounterMessage>::new("custom")).expect("init via setup-installed sharding")
  });

  assert!(registered_kinds.lock().contains(&String::from("custom")));
  system.terminate().expect("terminate");
}

#[test]
fn cluster_sharding_setup_installs_extension_id() {
  let registered_kinds = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let system = build_typed_system(registered_kinds);
  let extension = system.as_untyped().extended().extension(&ClusterShardingId::new());
  assert!(extension.is_some());
  system.terminate().expect("terminate");
}

struct TestActorRefProvider {
  system: ActorSystem,
}

impl TestActorRefProvider {
  fn new(system: ActorSystem) -> Self {
    Self { system }
  }
}

impl ActorRefProvider for TestActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    static SCHEMES: [ActorPathScheme; 1] = [ActorPathScheme::FraktorTcp];
    &SCHEMES
  }

  fn actor_ref(&mut self, _path: ActorPath) -> Result<ActorRef, ActorError> {
    Ok(ActorRef::with_system(Pid::new(1, 0), TestSender, &self.system.state()))
  }

  fn termination_signal(&self) -> TerminationSignal {
    TerminationSignal::already_terminated()
  }
}

struct TestSender;

impl ActorRefSender for TestSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}
