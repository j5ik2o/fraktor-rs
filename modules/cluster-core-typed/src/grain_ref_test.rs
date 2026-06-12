use alloc::{
  string::{String, ToString},
  vec,
};
use core::{any::Any, time::Duration};

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
    scheduler::{SchedulerConfig, SchedulerShared},
    setup::ActorSystemConfig,
  },
  serialization::SerializedMessage,
  system::{ActorSystem, TerminationSignal},
};
use fraktor_cluster_core_kernel_rs::{
  activation::{
    ActivatedKind, ClusterIdentity as KernelClusterIdentity, IdentityLookup, IdentitySetupError, LookupError,
    PlacementDecision, PlacementLocality, PlacementResolution,
  },
  cluster_provider::NoopClusterProvider,
  extension::{ClusterApi, ClusterExtension, ClusterExtensionConfig, ClusterExtensionInstaller},
  grain::{
    GrainCallError, GrainCallOptions, GrainCodec, GrainCodecError, GrainKey, GrainRef as KernelGrainRef,
    GrainRetryPolicy,
  },
};
use fraktor_utils_core_rs::{
  sync::{ArcShared, SharedAccess, SpinSyncMutex},
  time::TimerInstant,
};

use crate::GrainRef;

// ─── fixture ───────────────────────────────────────────────────────────────

/// テスト用 ActorSystem + ClusterExtension を構築する。
/// kernel の `build_system_with_extension_config` パターンを移植。
fn build_system_with_extension(
  send_counter: Option<&ArcShared<SpinSyncMutex<usize>>>,
  fail_on_send: bool,
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
      let provider = ActorRefProviderHandleShared::new(TestActorRefProvider::new(
        system.clone(),
        send_counter.clone(),
        fail_on_send,
      ));
      system.extended().register_actor_ref_provider(&provider)
    });
  let props = Props::from_fn(|| TestGuardian);
  let system = ActorSystem::create_from_props(&props, config).expect("build system");
  let extension = system.extended().extension_by_type::<ClusterExtension>().expect("cluster extension");
  (system, extension)
}

/// `GrainRef<M>` を構築するためのヘルパー。
fn build_grain_ref<M: Any + Send + Sync + 'static>(api: ClusterApi, kind: &str, id: &str) -> GrainRef<M> {
  let kernel_identity = KernelClusterIdentity::new(kind, id).expect("valid identity");
  let kernel_ref = KernelGrainRef::new(api, kernel_identity);
  GrainRef::from_kernel(kernel_ref)
}

fn run_scheduler(system: &ActorSystem, duration: Duration) {
  let scheduler: SchedulerShared = system.state().scheduler();
  let resolution = scheduler.with_read(|inner| inner.config().resolution());
  let resolution_ns = resolution.as_nanos().max(1);
  let ticks = duration.as_nanos().div_ceil(resolution_ns).max(1);
  let now = TimerInstant::from_ticks(ticks as u64, resolution);
  scheduler.with_write(|inner| {
    // retry scheduler 実行（戻り値は安全に無視、タイムアウト処理のみ目的）
    // SAFETY: run_due は schedule 済みのコールバックを実行するだけ。
    //   戻り値はスケジューラの内部診断（空ならOk(0)）であり、破棄しても契約は壊れない。
    let _ = inner.run_due(now);
  });
}

// ─── 往復検証: GrainRef を構築して identity/as_kernel/into_kernel をテストする ──

#[test]
fn roundtrip_identity_preserved() {
  // GrainRef<M> から identity() → as_kernel().identity() が一致するか確認する。
  let (system, ext) = build_system_with_extension(None, false);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("counter")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let kind = "counter";
  let entity_id = "abc-123";
  let grain: GrainRef<u32> = build_grain_ref(api, kind, entity_id);

  // identity() が typed ClusterIdentity<M> を返す
  let typed_id = grain.identity();
  assert_eq!(typed_id.kind(), kind);
  assert_eq!(typed_id.identity(), entity_id);

  // as_kernel() の identity と typed identity の kernel 表現が一致する
  assert_eq!(grain.as_kernel().identity(), typed_id.as_kernel());

  // into_kernel() 後も identity が保持される
  let kernel_ref = grain.into_kernel();
  assert_eq!(kernel_ref.identity().kind(), kind);
  assert_eq!(kernel_ref.identity().identity(), entity_id);
}

// ─── identity() が typed ClusterIdentity<M> を返す ─────────────────────────

#[test]
fn typed_identity_wraps_kernel() {
  let (system, ext) = build_system_with_extension(None, false);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("order")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let kind = "order";
  let entity_id = "order-42";
  let grain: GrainRef<()> = build_grain_ref(api, kind, entity_id);

  let typed_id = grain.identity();
  assert_eq!(typed_id.kind(), kind);
  assert_eq!(typed_id.identity(), entity_id);

  // kernel identity との参照一致
  let kernel_id = grain.as_kernel().identity();
  assert_eq!(typed_id.as_kernel(), kernel_id);
}

// ─── M 違いの識別が同一 kernel 宛先になる ─────────────────────────────────

#[test]
fn different_message_type_same_kernel_destination() {
  let (system, ext) = build_system_with_extension(None, false);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("worker")]).expect("setup kinds");

  let api_a = ClusterApi::try_from_system(&system).expect("cluster api a");
  let api_b = ClusterApi::try_from_system(&system).expect("cluster api b");
  let kind = "worker";
  let entity_id = "xyz-789";

  let grain_a: GrainRef<u32> = build_grain_ref(api_a, kind, entity_id);
  let grain_b: GrainRef<String> = build_grain_ref(api_b, kind, entity_id);

  // M が異なっても kernel identity は同一宛先
  assert_eq!(grain_a.as_kernel().identity(), grain_b.as_kernel().identity());
  assert_eq!(grain_a.identity().kind(), grain_b.identity().kind());
  assert_eq!(grain_a.identity().identity(), grain_b.identity().identity());
}

// ─── with_options が kernel へパススルーされる ──────────────────────────────
// 検証: retry policy を設定して応答しないターゲットへ request を送ると、
//       policy に従って送信回数が max_retries + 1 になる（options が kernel に到達した証明）。

#[test]
fn with_options_passes_through_to_kernel() {
  let send_counter = ArcShared::new(SpinSyncMutex::new(0usize));
  let (system, ext) = build_system_with_extension(Some(&send_counter), false);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("user")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let options = GrainCallOptions::new(Some(Duration::from_millis(1)), GrainRetryPolicy::Fixed {
    max_retries: 2,
    delay:       Duration::from_millis(1),
  });
  let kernel_identity = KernelClusterIdentity::new("user", "abc").expect("valid identity");
  let kernel_ref = KernelGrainRef::new(api, kernel_identity);
  // typed GrainRef::with_options を経由させる（typed 層の委譲そのものを検証する）
  let grain: GrainRef<()> = GrainRef::from_kernel(kernel_ref).with_options(options);

  // request は応答なしのターゲットへ送信する（エラー観察目的でなく送信回数観察のみ）
  let _response = grain.request::<()>(()).expect("request");
  run_scheduler(&system, Duration::from_millis(10));

  // send_counter: 初回 + 2 retry = 3（options retry policy が kernel に到達した証明）
  let sends = *send_counter.lock();
  assert_eq!(sends, 3, "options retry policy reached kernel: expected 3 sends");
}

// ─── with_codec が kernel へパススルーされる ────────────────────────────────
// 検証: 常に失敗する codec を typed GrainRef::with_codec で設定し、
//       request が GrainCallError::CodecFailed を返すことで codec が kernel に到達した証明。

#[test]
fn with_codec_reaches_kernel() {
  let (system, ext) = build_system_with_extension(None, false);
  ext.start_member().expect("start member");
  ext.setup_member_kinds(vec![ActivatedKind::new("item")]).expect("setup kinds");

  let api = ClusterApi::try_from_system(&system).expect("cluster api");
  let codec: ArcShared<dyn GrainCodec> = ArcShared::new(AlwaysFailCodec);

  let kernel_identity = KernelClusterIdentity::new("item", "item-1").expect("valid identity");
  let kernel_ref = KernelGrainRef::new(api, kernel_identity);
  let grain: GrainRef<()> = GrainRef::from_kernel(kernel_ref).with_codec(codec);

  // TypedAskResponse は Debug 未実装なので expect_err ではなく match で確認する
  match grain.request::<()>(()) {
    | Err(GrainCallError::CodecFailed(_)) => {},
    | Err(other) => panic!("expected CodecFailed, got: {other:?}"),
    | Ok(_) => panic!("expected Err(CodecFailed) but got Ok"),
  }
}

// ─── GrainRef<M> は Send + Sync ──────────────────────────────────────────────

fn _assert_send_sync<T: Send + Sync>() {}

#[test]
fn grain_ref_is_send_sync() {
  _assert_send_sync::<GrainRef<u32>>();
  _assert_send_sync::<GrainRef<String>>();
}

// ─── fixture: 失敗する GrainCodec ──────────────────────────────────────────

struct AlwaysFailCodec;

impl GrainCodec for AlwaysFailCodec {
  fn encode(&self, _message: &AnyMessage) -> Result<SerializedMessage, GrainCodecError> {
    Err(GrainCodecError::EncodeFailed { reason: String::from("always fails for test") })
  }

  fn decode(&self, _payload: &SerializedMessage) -> Result<AnyMessage, GrainCodecError> {
    Err(GrainCodecError::DecodeFailed { reason: String::from("always fails for test") })
  }
}

// ─── fixture: TestGuardian ────────────────────────────────────────────────

struct TestGuardian;

impl Actor for TestGuardian {
  fn receive(&mut self, _context: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

// ─── fixture: StaticIdentityLookup ──────────────────────────────────────

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

// ─── fixture: TestActorRefProvider ────────────────────────────────────────

struct TestActorRefProvider {
  system:       ActorSystem,
  counter:      Option<ArcShared<SpinSyncMutex<usize>>>,
  fail_on_send: bool,
}

impl TestActorRefProvider {
  fn new(system: ActorSystem, counter: Option<ArcShared<SpinSyncMutex<usize>>>, fail_on_send: bool) -> Self {
    Self { system, counter, fail_on_send }
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
      TestSender { counter: self.counter.clone(), fail_on_send: self.fail_on_send },
      &self.system.state(),
    ))
  }

  fn termination_signal(&self) -> TerminationSignal {
    TerminationSignal::already_terminated()
  }
}

struct TestSender {
  counter:      Option<ArcShared<SpinSyncMutex<usize>>>,
  fail_on_send: bool,
}

impl ActorRefSender for TestSender {
  fn send(&mut self, _message: AnyMessage) -> Result<SendOutcome, SendError> {
    if self.fail_on_send {
      return Err(SendError::timeout(AnyMessage::new(())));
    }
    if let Some(counter) = &self.counter {
      *counter.lock() += 1;
    }
    Ok(SendOutcome::Delivered)
  }
}
