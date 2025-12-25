use alloc::string::String;

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
  scheduler::{ManualTestDriver, SchedulerConfig, TickDriverConfig},
  system::{ActorRefProvider, ActorRefProviderSharedGeneric, ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_utils_rs::core::runtime_toolbox::NoStdToolbox;

use crate::core::{
  ActivatedKind, ClusterApiGeneric, ClusterExtensionConfig, ClusterExtensionGeneric, ClusterExtensionInstaller,
  ClusterIdentity, GrainContextGeneric, GrainContextScope, GrainKey, IdentityLookup, IdentitySetupError, LookupError,
  NoopClusterProvider, PlacementDecision, PlacementLocality, PlacementResolution,
};

#[test]
fn grain_context_scope_finishes_and_releases_context() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let context = GrainContextGeneric::new(identity, api);

  let mut scope = GrainContextScope::new(context);
  assert!(scope.is_active());
  assert!(scope.context().is_some());

  scope.finish();
  assert!(!scope.is_active());
  assert!(scope.context().is_none());
}

fn build_system_with_extension<F>(
  identity_lookup_factory: F,
) -> (ActorSystemGeneric<NoStdToolbox>, fraktor_utils_rs::core::sync::ArcShared<ClusterExtensionGeneric<NoStdToolbox>>)
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
