use fraktor_actor_rs::core::{
  actor::{
    Actor, Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::{ActorRefGeneric, ActorRefSender, ActorRefSenderSharedGeneric, SendOutcome},
  },
  error::ActorError,
  extension::ExtensionInstallers,
  messaging::AnyMessageGeneric,
  props::PropsGeneric,
  scheduler::{
    SchedulerConfig,
    tick_driver::{ManualTestDriver, TickDriverConfig},
  },
  system::{
    ActorSystemConfigGeneric, ActorSystemGeneric,
    provider::{ActorRefProvider, ActorRefProviderSharedGeneric},
  },
};
use fraktor_cluster_rs::core::{
  ActivatedKind, ClusterApiGeneric, ClusterExtensionConfig, ClusterExtensionGeneric, ClusterExtensionInstaller,
  ClusterIdentity, GrainKey, IdentityLookup, IdentitySetupError, LookupError, PlacementDecision, PlacementLocality,
  PlacementResolution, cluster_provider::NoopClusterProvider,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::NoStdToolbox,
  sync::{ArcShared, SharedAccess},
};

#[test]
fn cluster_api_request_flow_works() {
  let (system, extension) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  extension.start_member().expect("start member");
  extension.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let response = api.request(&identity, AnyMessageGeneric::new(()), None).expect("request ok");
  assert!(!response.future().with_read(|inner| inner.is_ready()));
}

fn build_system_with_extension<F>(
  identity_lookup_factory: F,
) -> (ActorSystemGeneric<NoStdToolbox>, ArcShared<ClusterExtensionGeneric<NoStdToolbox>>)
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
  let config = ActorSystemConfigGeneric::default()
    .with_scheduler_config(scheduler_config)
    .with_tick_driver(tick_driver)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(|system: &ActorSystemGeneric<NoStdToolbox>| {
      let provider = ActorRefProviderSharedGeneric::new(TestActorRefProvider::new(system.clone()));
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = PropsGeneric::from_fn(|| TestGuardian);
  let system = ActorSystemGeneric::new_with_config(&props, &config).expect("build system");
  let extension =
    system.extended().extension_by_type::<ClusterExtensionGeneric<NoStdToolbox>>().expect("cluster extension");
  (system, extension)
}

struct TestGuardian;

impl Actor<NoStdToolbox> for TestGuardian {
  fn receive(
    &mut self,
    _context: &mut fraktor_actor_rs::core::actor::ActorContextGeneric<'_, NoStdToolbox>,
    _message: fraktor_actor_rs::core::messaging::AnyMessageViewGeneric<'_, NoStdToolbox>,
  ) -> Result<(), fraktor_actor_rs::core::error::ActorError> {
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
    let decision =
      PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now };
    let pid = format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution { decision, locality: PlacementLocality::Local, pid })
  }
}

struct TestActorRefProvider {
  system: ActorSystemGeneric<NoStdToolbox>,
}

impl TestActorRefProvider {
  fn new(system: ActorSystemGeneric<NoStdToolbox>) -> Self {
    Self { system }
  }
}

impl ActorRefProvider<NoStdToolbox> for TestActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    static SCHEMES: [ActorPathScheme; 1] = [ActorPathScheme::FraktorTcp];
    &SCHEMES
  }

  fn actor_ref(&mut self, _path: ActorPath) -> Result<ActorRefGeneric<NoStdToolbox>, ActorError> {
    let sender = ActorRefSenderSharedGeneric::new(TestSender);
    Ok(ActorRefGeneric::from_shared(Pid::new(1, 0), sender, &self.system.state()))
  }
}

struct TestSender;

impl ActorRefSender<NoStdToolbox> for TestSender {
  fn send(
    &mut self,
    _message: AnyMessageGeneric<NoStdToolbox>,
  ) -> Result<SendOutcome, fraktor_actor_rs::core::error::SendError<NoStdToolbox>> {
    Ok(SendOutcome::Delivered)
  }
}
