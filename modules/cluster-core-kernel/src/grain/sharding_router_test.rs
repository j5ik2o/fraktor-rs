use alloc::{
  boxed::Box,
  string::{String, ToString},
};

use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;
use fraktor_actor_core_kernel_rs::{
  actor::{
    Actor, ActorContext, Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    actor_ref_provider::{ActorRefProvider, ActorRefProviderHandleShared},
    error::{ActorError, SendError},
    extension::ExtensionInstallers,
    messaging::{AnyMessage, AnyMessageView},
    props::Props,
    scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::{ActorSystem, TerminationSignal},
};
use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use super::ShardingRouter;
use crate::{
  ClusterApi, ClusterExtension, ClusterExtensionConfig,
  activation::{
    ActivatedKind, ClusterIdentity, ClusterIdentityError, IdentityLookup, IdentitySetupError, LookupError,
    PlacementDecision, PlacementLocality, PlacementResolution,
  },
  cluster_provider::NoopClusterProvider,
  extension::ClusterExtensionInstaller,
  grain::{
    GrainCallError, GrainKey, GrainRef, HashCodeMessageExtractor, ShardingDispatchError, ShardingEnvelope,
    ShardingMessageExtractor,
  },
};

#[test]
fn grain_ref_for_resolves_same_destination_as_explicit_construction() {
  let (system, ext) = build_system(SendBehavior::Ok, None);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let extractor = HashCodeMessageExtractor::<u32>::new(4).expect("extractor");
  let router = ShardingRouter::new(api.clone(), "user", extractor);
  let envelope = ShardingEnvelope::new("abc", 7u32);

  let routed = router.grain_ref_for(&envelope).expect("grain ref");

  let explicit = GrainRef::new(api, ClusterIdentity::new("user", "abc").expect("identity"));
  assert_eq!(routed.identity(), explicit.identity());
  // 解決される宛先も明示構築と同一であることを確認する
  let routed_resolved = routed.get().expect("resolved");
  let explicit_resolved = explicit.get().expect("resolved");
  assert_eq!(routed_resolved.actor_ref.pid(), explicit_resolved.actor_ref.pid());
}

#[test]
fn underivable_entity_id_rejects_dispatch() {
  let (system, ext) = build_system(SendBehavior::Ok, None);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let router = ShardingRouter::new(api, "user", UnderivableExtractor);
  let envelope = ShardingEnvelope::new("abc", 7u32);

  let error = match router.grain_ref_for(&envelope) {
    | Ok(_) => panic!("grain_ref_for should reject"),
    | Err(err) => err,
  };
  assert_eq!(error, ShardingDispatchError::EntityIdUnderivable);

  let sender_ref = noop_sender_ref(&system);
  let error = router.tell_with_sender(ShardingEnvelope::new("abc", 7u32), &sender_ref).expect_err("must reject");
  assert_eq!(error, ShardingDispatchError::EntityIdUnderivable);
}

#[test]
fn invalid_identity_rejects_dispatch() {
  let (system, ext) = build_system(SendBehavior::Ok, None);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let extractor = HashCodeMessageExtractor::<u32>::new(4).expect("extractor");
  let router = ShardingRouter::new(api, "user", extractor);
  // 空 entity id は ClusterIdentity の検証規則で拒否される
  let envelope = ShardingEnvelope::new("", 7u32);

  let error = match router.grain_ref_for(&envelope) {
    | Ok(_) => panic!("grain_ref_for should reject"),
    | Err(err) => err,
  };
  assert_eq!(error, ShardingDispatchError::InvalidIdentity(ClusterIdentityError::EmptyIdentity));
}

#[test]
fn tell_with_sender_delegates_to_existing_grain_path() {
  let send_counter = ArcShared::new(SpinSyncMutex::new(0usize));
  let (system, ext) = build_system(SendBehavior::Ok, Some(&send_counter));
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let extractor = HashCodeMessageExtractor::<u32>::new(4).expect("extractor");
  let router = ShardingRouter::new(api, "user", extractor);
  let sender_ref = noop_sender_ref(&system);

  router.tell_with_sender(ShardingEnvelope::new("abc", 7u32), &sender_ref).expect("tell");

  assert_eq!(*send_counter.lock(), 1);
}

#[test]
fn tell_with_sender_propagates_call_failure() {
  let (system, ext) = build_system(SendBehavior::Fail, None);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let extractor = HashCodeMessageExtractor::<u32>::new(4).expect("extractor");
  let router = ShardingRouter::new(api, "user", extractor);
  let sender_ref = noop_sender_ref(&system);

  let error = router.tell_with_sender(ShardingEnvelope::new("abc", 7u32), &sender_ref).expect_err("must fail");
  assert!(matches!(error, ShardingDispatchError::Call(GrainCallError::RequestFailed(_))));
}

#[test]
fn request_delegates_and_unwraps_inner_message() {
  let (system, ext) = build_system(SendBehavior::EchoPayload, None);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let extractor = HashCodeMessageExtractor::<String>::new(4).expect("extractor");
  let router = ShardingRouter::new(api, "user", extractor);

  let response = router.request(ShardingEnvelope::new("abc", String::from("ping"))).expect("request");

  let result = response.future().with_write(|inner| inner.try_take()).expect("future ready");
  let reply = result.expect("reply ok");
  // EchoPayload は受信した payload をそのまま返す。unwrap 済みの内部メッセージ
  // （String）が既存経路へ渡っていることを応答で確認する
  let reply_text = reply.payload().downcast_ref::<String>().expect("reply string");
  assert_eq!(reply_text, "ping");
}

#[test]
fn request_future_delegates_to_existing_grain_path() {
  let (system, ext) = build_system(SendBehavior::EchoPayload, None);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let extractor = HashCodeMessageExtractor::<String>::new(4).expect("extractor");
  let router = ShardingRouter::new(api, "user", extractor);

  let future = router.request_future(ShardingEnvelope::new("abc", String::from("ping"))).expect("request future");

  let result = future.with_write(|inner| inner.try_take()).expect("future ready");
  let reply = result.expect("reply ok");
  let reply_text = reply.payload().downcast_ref::<String>().expect("reply string");
  assert_eq!(reply_text, "ping");
}

/// entity id を常に導出不能とするテストローカル extractor。
struct UnderivableExtractor;

impl ShardingMessageExtractor<ShardingEnvelope<u32>, u32> for UnderivableExtractor {
  fn entity_id(&self, _message: &ShardingEnvelope<u32>) -> Option<String> {
    None
  }

  fn shard_id(&self, _entity_id: &str) -> String {
    String::from("0")
  }

  fn unwrap_message(&self, message: ShardingEnvelope<u32>) -> u32 {
    message.into_message()
  }
}

fn noop_sender_ref(system: &ActorSystem) -> ActorRef {
  ActorRef::with_system(Pid::new(99, 0), NoopSender, &system.state())
}

fn build_system(
  send_behavior: SendBehavior,
  send_counter: Option<&ArcShared<SpinSyncMutex<usize>>>,
) -> (ActorSystem, ArcShared<ClusterExtension>) {
  let scheduler_config = SchedulerConfig::default().with_runner_api_enabled(true);
  let cluster_config = ClusterExtensionConfig::new().with_advertised_address("node1:8080");
  let cluster_installer = ClusterExtensionInstaller::new(cluster_config, |_event_stream, _block_list, _address| {
    Box::new(NoopClusterProvider::new())
  })
  .with_identity_lookup_factory(|| Box::new(StaticIdentityLookup::new("node1:8080")));
  let extensions = ExtensionInstallers::default().with_extension_installer(cluster_installer);
  let send_counter = send_counter.cloned();
  let config = ActorSystemConfig::new(TestTickDriver::default())
    .with_scheduler_config(scheduler_config)
    .with_extension_installers(extensions)
    .with_actor_ref_provider_installer(move |system: &ActorSystem| {
      let actor_ref_provider_handle_shared = ActorRefProviderHandleShared::new(TestActorRefProvider::new(
        system.clone(),
        send_counter.clone(),
        send_behavior,
      ));
      system.extended().register_actor_ref_provider(&actor_ref_provider_handle_shared)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::create_from_props(&props, config).expect("build system");
  let extension = system.extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  (system, extension)
}

#[derive(Clone, Copy)]
enum SendBehavior {
  Ok,
  Fail,
  EchoPayload,
}

struct TestGuardian;

impl Actor for TestGuardian {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
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
  system:   ActorSystem,
  counter:  Option<ArcShared<SpinSyncMutex<usize>>>,
  behavior: SendBehavior,
}

impl TestActorRefProvider {
  fn new(system: ActorSystem, counter: Option<ArcShared<SpinSyncMutex<usize>>>, behavior: SendBehavior) -> Self {
    Self { system, counter, behavior }
  }
}

impl ActorRefProvider for TestActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    static SCHEMES: [ActorPathScheme; 1] = [ActorPathScheme::FraktorTcp];
    &SCHEMES
  }

  fn actor_ref(&mut self, _path: ActorPath) -> Result<ActorRef, ActorError> {
    Ok(ActorRef::with_system(
      Pid::new(1, 0),
      TestSender { counter: self.counter.clone(), behavior: self.behavior },
      &self.system.state(),
    ))
  }

  fn termination_signal(&self) -> TerminationSignal {
    TerminationSignal::already_terminated()
  }
}

struct TestSender {
  counter:  Option<ArcShared<SpinSyncMutex<usize>>>,
  behavior: SendBehavior,
}

impl ActorRefSender for TestSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    if matches!(self.behavior, SendBehavior::Fail) {
      return Err(SendError::timeout(AnyMessage::new(())));
    }
    if matches!(self.behavior, SendBehavior::EchoPayload)
      && let Some(mut sender) = message.sender().cloned()
    {
      // 受信した payload をそのまま返信する（unwrap 済み内部メッセージの検証用）
      sender.tell(message.clone());
    }
    if let Some(counter) = &self.counter {
      *counter.lock() += 1;
    }
    Ok(SendOutcome::Delivered)
  }
}

struct NoopSender;

impl ActorRefSender for NoopSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    Ok(SendOutcome::Delivered)
  }
}
