use alloc::string::String;
use core::time::Duration;

use fraktor_actor_rs::core::{
  actor_prim::{
    Actor, Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::{ActorRefGeneric, ActorRefSender, ActorRefSenderSharedGeneric, SendOutcome},
  },
  dispatch::scheduler::{ManualTestDriver, SchedulerConfig, SchedulerSharedGeneric, TickDriverConfig},
  error::ActorError,
  extension::ExtensionInstallers,
  messaging::AnyMessageGeneric,
  props::PropsGeneric,
  system::{ActorRefProvider, ActorRefProviderSharedGeneric, ActorSystemConfigGeneric, ActorSystemGeneric},
};
use fraktor_utils_rs::core::{
  runtime_toolbox::NoStdToolbox,
  sync::{ArcShared, SharedAccess},
  time::TimerInstant,
};

use crate::core::{
  ActivatedKind, ClusterApiError, ClusterApiGeneric, ClusterExtensionConfig, ClusterExtensionGeneric,
  ClusterExtensionInstaller, ClusterIdentity, ClusterRequestError, ClusterResolveError, GrainKey, IdentityLookup,
  IdentitySetupError, LookupError, NoopClusterProvider, NoopIdentityLookup, PlacementDecision, PlacementLocality,
  PlacementResolution,
};

#[test]
fn try_from_system_fails_when_extension_missing() {
  let system = ActorSystemGeneric::<NoStdToolbox>::new_empty();
  match ClusterApiGeneric::try_from_system(&system) {
    | Ok(_) => panic!("extension should be missing"),
    | Err(err) => assert_eq!(err, ClusterApiError::ExtensionNotInstalled),
  }
}

#[test]
fn try_from_system_returns_existing_extension() {
  let (system, _ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));

  let first = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let second = ClusterApiGeneric::try_from_system(&system).expect("cluster api");

  assert!(ArcShared::ptr_eq(&first.extension, &second.extension));
}

#[test]
fn get_fails_when_cluster_not_started() {
  let (system, _ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("not started");
  assert_eq!(err, ClusterResolveError::ClusterNotStarted);
}

#[test]
fn get_fails_when_kind_not_registered() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("kind not registered");
  assert_eq!(err, ClusterResolveError::KindNotRegistered { kind: "user".to_string() });
}

#[test]
fn get_fails_on_invalid_pid_format() {
  let (system, ext) = build_system_with_extension(|| Box::new(InvalidIdentityLookup));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let err = api.get(&identity).expect_err("invalid pid");
  assert!(matches!(err, ClusterResolveError::InvalidPidFormat { .. }));
}

#[test]
fn get_resolves_actor_ref_for_registered_kind() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");

  let actor_ref = api.get(&identity).expect("resolved actor ref");
  assert_eq!(actor_ref.pid(), Pid::new(1, 0));
}

#[test]
fn request_returns_error_when_lookup_fails() {
  let (system, ext) = build_system_with_extension(|| Box::new(NoopIdentityLookup::new()));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  match api.request(&identity, AnyMessageGeneric::new(()), None) {
    | Ok(_) => panic!("lookup should fail"),
    | Err(err) => assert_eq!(err, ClusterRequestError::ResolveFailed(ClusterResolveError::LookupFailed)),
  }
}

#[test]
fn request_returns_ok_without_timeout() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let response = api.request(&identity, AnyMessageGeneric::new(()), None).expect("request ok");

  assert!(!response.future().with_read(|inner| inner.is_ready()));
}

#[test]
fn request_future_completes_with_timeout_payload() {
  let (system, ext) = build_system_with_extension(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApiGeneric::try_from_system(&system).expect("cluster api");
  let identity = ClusterIdentity::new("user", "abc").expect("identity");
  let future =
    api.request_future(&identity, AnyMessageGeneric::new(()), Some(Duration::from_millis(1))).expect("request");

  assert!(!future.with_read(|inner| inner.is_ready()));

  run_scheduler(&system, Duration::from_millis(1));

  let message = future.with_write(|inner| inner.try_take()).expect("timeout payload");
  let payload = message.payload().downcast_ref::<ClusterRequestError>().expect("payload");
  assert_eq!(payload, &ClusterRequestError::Timeout);
}

fn run_scheduler(system: &ActorSystemGeneric<NoStdToolbox>, duration: Duration) {
  let scheduler: SchedulerSharedGeneric<NoStdToolbox> = system.state().scheduler();
  let resolution = scheduler.with_read(|inner| inner.config().resolution());
  let resolution_ns = resolution.as_nanos().max(1);
  let ticks = duration.as_nanos().div_ceil(resolution_ns).max(1);
  let now = TimerInstant::from_ticks(ticks as u64, resolution);
  scheduler.with_write(|inner| {
    let _ = inner.run_due(now);
  });
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
    _context: &mut fraktor_actor_rs::core::actor_prim::ActorContextGeneric<'_, NoStdToolbox>,
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
    let pid = format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }
}

struct InvalidIdentityLookup;

impl IdentityLookup for InvalidIdentityLookup {
  fn setup_member(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn setup_client(&mut self, _kinds: &[ActivatedKind]) -> Result<(), IdentitySetupError> {
    Ok(())
  }

  fn resolve(&mut self, key: &GrainKey, now: u64) -> Result<PlacementResolution, LookupError> {
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: "invalid".to_string(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid:      "invalid_pid".to_string(),
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
