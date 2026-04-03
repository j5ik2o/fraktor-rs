use alloc::string::String;

use fraktor_actor_rs::core::kernel::{
  actor::{
    Actor, Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::{ActorRef, ActorRefSender, ActorRefSenderShared, SendOutcome},
    actor_ref_provider::{ActorRefProvider, ActorRefProviderShared},
    error::ActorError,
    extension::ExtensionInstallers,
    messaging::AnyMessage,
    props::Props,
    scheduler::{
      SchedulerConfig,
      tick_driver::{ManualTestDriver, TickDriverConfig},
    },
    setup::ActorSystemConfig,
  },
  system::ActorSystem,
};

use crate::core::{
  ClusterApi, ClusterExtension, ClusterExtensionConfig, ClusterExtensionInstaller,
  cluster_provider::NoopClusterProvider,
  grain::{GrainContext, GrainContextImpl, GrainKey},
  identity::{ClusterIdentity, IdentityLookup, IdentitySetupError, LookupError},
  placement::{ActivatedKind, PlacementDecision, PlacementLocality, PlacementResolution},
};

#[test]
fn grain_context_exposes_identity_and_cluster() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let context = GrainContextImpl::new(identity.clone(), api);

  assert_eq!(context.kind(), "user");
  assert_eq!(context.identity(), "abc");

  let actor_ref = context.cluster().get(&identity).expect("resolved");
  assert_eq!(actor_ref.pid(), Pid::new(1, 0));
}

fn build_system_with_extension<F>(
  identity_lookup_factory: F,
) -> (ActorSystem, fraktor_utils_rs::core::sync::ArcShared<ClusterExtension>)
where
  F: Fn() -> Box<dyn IdentityLookup> + Send + Sync + 'static, {
  let tick_driver = TickDriverConfig::manual(ManualTestDriver::new());
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  })
  .with_identity_lookup_factory(identity_lookup_factory);
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let config = ActorSystemConfig::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystem| {
      let provider = ActorRefProviderShared::new(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::new_with_config(&props, &config).expect("build system");
  let extension = system.extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  (system, extension)
}

struct TestGuardian;

impl Actor for TestGuardian {
  fn receive(
    &mut self,
    _context: &mut fraktor_actor_rs::core::kernel::actor::ActorContext<'_>,
    _message: fraktor_actor_rs::core::kernel::actor::messaging::AnyMessageView<'_>,
  ) -> Result<(), ActorError> {
    Ok(())
  }
}

struct StaticIdentityLookup {
  authority: String,
}

impl StaticIdentityLookup {
  fn new(authority: &str) -> Self {
    Self { authority: authority.to_string() }
  }
}

impl IdentityLookup for StaticIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    let pid = format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }
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
    let sender = ActorRefSenderShared::new(TestSender);
    Ok(ActorRef::from_shared(Pid::new(1, 0), sender, &self.system.state()))
  }

  fn termination_future(&self) -> fraktor_actor_rs::core::kernel::util::futures::ActorFutureShared<()> {
    fraktor_actor_rs::core::kernel::util::futures::ActorFutureShared::new()
  }
}

struct TestSender;

impl ActorRefSender for TestSender {
  fn send(
    &mut self,
    _message: AnyMessage,
  ) -> Result<SendOutcome, fraktor_actor_rs::core::kernel::actor::error::SendError> {
    Ok(SendOutcome::Delivered)
  }
}
