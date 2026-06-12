// typed grain facade の統合テスト（タスク 4.1）。
// 宣言点（GrainTypeKey）→ 取得（Cluster::grain_ref_for）→ 呼び出し往復の経路を検証する。
// 呼び出しオプション・codec のパススルー検証は grain_ref_test.rs（2.2）が所有する。

use alloc::{
  string::{String, ToString},
  vec,
};

extern crate alloc;

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
    scheduler::SchedulerConfig,
    setup::ActorSystemConfig,
  },
  system::{ActorSystem, TerminationSignal},
};
use fraktor_actor_core_typed_rs::{
  TypedActorRef, TypedActorSystem, TypedProps,
  dsl::{Behaviors, TypedAskError},
};
use fraktor_cluster_core_kernel_rs::{
  activation::{
    ActivatedKind, IdentityLookup, IdentitySetupError, LookupError, PlacementDecision, PlacementLocality,
    PlacementResolution,
  },
  cluster_provider::NoopClusterProvider,
  extension::{ClusterApiError, ClusterExtension, ClusterExtensionConfig, ClusterExtensionInstaller},
  grain::{GrainCallError, GrainKey},
};
use fraktor_cluster_core_typed_rs::{Cluster, GrainTypeKey};
use fraktor_utils_core_rs::sync::{ArcShared, SpinSyncMutex};

// ─── fixture: メッセージ型 ───────────────────────────────────────────────────

#[derive(Debug)]
struct UserMessage;

// ─── fixture: StaticIdentityLookup ──────────────────────────────────────────

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
    let pid = alloc::format!("{}::{}", self.authority, key.value());
    Ok(PlacementResolution {
      decision: PlacementDecision { key: key.clone(), authority: self.authority.clone(), observed_at: now },
      locality: PlacementLocality::Remote,
      pid,
    })
  }
}

// ─── fixture: 応答する ActorRefProvider ─────────────────────────────────────

struct TestActorRefProvider {
  system:        ActorSystem,
  send_counter:  Option<ArcShared<SpinSyncMutex<usize>>>,
  reply_on_send: bool,
  fail_on_send:  bool,
}

impl ActorRefProvider for TestActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    static SCHEMES: [ActorPathScheme; 1] = [ActorPathScheme::FraktorTcp];
    &SCHEMES
  }

  fn actor_ref(&mut self, _path: ActorPath) -> Result<ActorRef, ActorError> {
    Ok(ActorRef::with_system(
      Pid::new(1, 0),
      TestSender {
        send_counter:  self.send_counter.clone(),
        reply_on_send: self.reply_on_send,
        fail_on_send:  self.fail_on_send,
      },
      &self.system.state(),
    ))
  }

  fn termination_signal(&self) -> TerminationSignal {
    TerminationSignal::already_terminated()
  }
}

struct TestSender {
  send_counter:  Option<ArcShared<SpinSyncMutex<usize>>>,
  reply_on_send: bool,
  fail_on_send:  bool,
}

impl ActorRefSender for TestSender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    // fail_on_send: 送達失敗を起こす（失敗伝搬の検証用）
    if self.fail_on_send {
      return Err(SendError::timeout(AnyMessage::new(())));
    }
    // reply_on_send: 送信者へ String("reply") を返す（ask future を完了させる）
    if self.reply_on_send
      && let Some(mut sender) = message.sender().cloned()
    {
      let reply = AnyMessage::new(String::from("reply"));
      sender.tell(reply);
    }
    if let Some(counter) = &self.send_counter {
      *counter.lock() += 1;
    }
    Ok(SendOutcome::Delivered)
  }
}

// ─── fixture: typed bootstrap ────────────────────────────────────────────────

// cluster 拡張導入済みの TypedActorSystem を構築し、member 起動と kind 登録まで行う。
fn build_typed_system_with_extension(
  send_counter: Option<&ArcShared<SpinSyncMutex<usize>>>,
  reply_on_send: bool,
) -> (TypedActorSystem<UserMessage>, ArcShared<ClusterExtension>) {
  build_typed_system_with_extension_config(send_counter, reply_on_send, false)
}

// 送達失敗（fail_on_send）も指定できる完全版。
fn build_typed_system_with_extension_config(
  send_counter: Option<&ArcShared<SpinSyncMutex<usize>>>,
  reply_on_send: bool,
  fail_on_send: bool,
) -> (TypedActorSystem<UserMessage>, ArcShared<ClusterExtension>) {
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
      let provider = ActorRefProviderHandleShared::new(TestActorRefProvider {
        system: system.clone(),
        send_counter: send_counter.clone(),
        reply_on_send,
        fail_on_send,
      });
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = TypedProps::<UserMessage>::from_behavior_factory(Behaviors::ignore);
  let system = TypedActorSystem::create_from_props(&props, config).expect("typed system");
  let extension = system.as_untyped().extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  extension.start_member().expect("start member");
  extension.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");
  (system, extension)
}

// ─── テスト: 宣言点 → 取得 → request 往復（要件 2.4, 3.1） ───────────────────

#[test]
fn grain_request_round_trip_returns_typed_response() {
  let (system, _ext) = build_typed_system_with_extension(None, true);

  let cluster = Cluster::get(&system).expect("cluster facade");
  let key = GrainTypeKey::<UserMessage>::new("user");
  let identity = key.identity_for("abc").expect("identity");
  let grain = cluster.grain_ref_for(&identity);

  let response = grain.request::<String>(UserMessage).expect("request");
  let mut future = response.future().clone();
  assert!(future.is_ready(), "reply should complete the future synchronously");
  let reply = future.try_take().expect("future ready").expect("typed reply");
  assert_eq!(reply, "reply");
}

// ─── テスト: 拡張未導入の取得拒否（要件 3.2） ────────────────────────────────

#[test]
fn cluster_get_rejects_system_without_cluster_extension() {
  let props = TypedProps::<UserMessage>::from_behavior_factory(Behaviors::ignore);
  let config = ActorSystemConfig::new(TestTickDriver::default());
  let system = TypedActorSystem::create_from_props(&props, config).expect("typed system");

  let result = Cluster::get(&system);
  assert!(
    matches!(result, Err(ClusterApiError::ExtensionNotInstalled)),
    "expected ExtensionNotInstalled, got: {:?}",
    result.err()
  );
}

// ─── テスト: tell の送達（要件 2.3） ─────────────────────────────────────────

#[test]
fn grain_tell_with_sender_delivers_message() {
  let send_counter = ArcShared::new(SpinSyncMutex::new(0usize));
  let (system, _ext) = build_typed_system_with_extension(Some(&send_counter), false);

  let cluster = Cluster::get(&system).expect("cluster facade");
  let key = GrainTypeKey::<UserMessage>::new("user");
  let identity = key.identity_for("abc").expect("identity");
  let grain = cluster.grain_ref_for(&identity);

  let sender: TypedActorRef<UserMessage> = TypedActorRef::from_untyped(ActorRef::null());
  grain.tell_with_sender(UserMessage, &sender).expect("tell");

  assert_eq!(*send_counter.lock(), 1, "tell should deliver exactly one message");
}

// ─── テスト: tell の送信失敗伝搬（要件 2.3） ─────────────────────────────────

#[test]
fn grain_tell_with_sender_propagates_send_failure() {
  let (system, _ext) = build_typed_system_with_extension_config(None, false, true);

  let cluster = Cluster::get(&system).expect("cluster facade");
  let key = GrainTypeKey::<UserMessage>::new("user");
  let identity = key.identity_for("abc").expect("identity");
  let grain = cluster.grain_ref_for(&identity);

  let sender: TypedActorRef<UserMessage> = TypedActorRef::from_untyped(ActorRef::null());
  // 送達失敗は GrainCallError::RequestFailed(SendFailed) として呼び出し元へ返る
  match grain.tell_with_sender(UserMessage, &sender) {
    | Err(GrainCallError::RequestFailed(_)) => {},
    | Err(other) => panic!("expected RequestFailed, got: {other:?}"),
    | Ok(()) => panic!("expected Err(RequestFailed) but tell succeeded"),
  }
}

// ─── テスト: request_future の応答（要件 2.5） ───────────────────────────────

#[test]
fn grain_request_future_resolves_typed_reply() {
  let (system, _ext) = build_typed_system_with_extension(None, true);

  let cluster = Cluster::get(&system).expect("cluster facade");
  let key = GrainTypeKey::<UserMessage>::new("user");
  let identity = key.identity_for("abc").expect("identity");
  let grain = cluster.grain_ref_for(&identity);

  let mut future = grain.request_future::<String>(UserMessage).expect("request_future");
  assert!(future.is_ready(), "reply should complete the future synchronously");
  let reply = future.try_take().expect("future ready").expect("typed reply");
  assert_eq!(reply, "reply");
}

// ─── テスト: 宛先解決失敗の伝搬（要件 2.6） ──────────────────────────────────

#[test]
fn grain_request_to_unregistered_kind_fails_with_resolve_failed() {
  let (system, _ext) = build_typed_system_with_extension(None, true);

  let cluster = Cluster::get(&system).expect("cluster facade");
  // "ghost" は setup_member_kinds で登録していない kind
  let key = GrainTypeKey::<UserMessage>::new("ghost");
  let identity = key.identity_for("abc").expect("identity");
  let grain = cluster.grain_ref_for(&identity);

  match grain.request::<String>(UserMessage) {
    | Err(GrainCallError::ResolveFailed(_)) => {},
    | Err(other) => panic!("expected ResolveFailed, got: {other:?}"),
    | Ok(_) => panic!("expected Err(ResolveFailed) but request succeeded"),
  }
}

// ─── テスト: 応答型不一致の伝搬（要件 2.6） ──────────────────────────────────

#[test]
fn grain_request_type_mismatch_surfaces_on_take() {
  let (system, _ext) = build_typed_system_with_extension(None, true);

  let cluster = Cluster::get(&system).expect("cluster facade");
  let key = GrainTypeKey::<UserMessage>::new("user");
  let identity = key.identity_for("abc").expect("identity");
  let grain = cluster.grain_ref_for(&identity);

  // 実際の応答は String だが、呼び出し側は u32 を表明する
  let mut future = grain.request_future::<u32>(UserMessage).expect("request_future");
  assert!(future.is_ready());
  let result = future.try_take().expect("future ready");
  assert!(
    matches!(result, Err(TypedAskError::TypeMismatch)),
    "expected TypeMismatch for wrong response type assertion"
  );
}
