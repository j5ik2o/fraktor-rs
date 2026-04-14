//! Tests for ActorSelectionResolver

use core::time::Duration;
use std::{env, thread, time::Instant};

use fraktor_utils_core_rs::core::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use crate::core::kernel::{
  actor::{
    Actor, ActorContext, ChildRef, Pid,
    actor_path::{ActorPath, ActorPathError, ActorPathParts, PathResolutionError},
    actor_ref::{ActorRef, NullSender},
    actor_selection::{ActorSelection, ActorSelectionError, ActorSelectionResolver},
    error::ActorError,
    messaging::{ActorIdentity, AnyMessage, AnyMessageView},
    props::Props,
    scheduler::tick_driver::TestTickDriver,
    setup::ActorSystemConfig,
  },
  system::{
    ActorSystem,
    remote::{RemoteAuthorityError, RemoteAuthorityRegistry},
  },
};

struct NoopActor;

impl Actor for NoopActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct SelectionProbeActor {
  messages: ArcShared<SpinSyncMutex<Vec<String>>>,
  senders:  ArcShared<SpinSyncMutex<Vec<Option<Pid>>>>,
}

impl SelectionProbeActor {
  fn new(messages: ArcShared<SpinSyncMutex<Vec<String>>>, senders: ArcShared<SpinSyncMutex<Vec<Option<Pid>>>>) -> Self {
    Self { messages, senders }
  }
}

impl Actor for SelectionProbeActor {
  fn receive(&mut self, ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(text) = message.downcast_ref::<String>() {
      self.messages.lock().push(text.clone());
      self.senders.lock().push(ctx.sender().map(|sender| sender.pid()));
    }
    Ok(())
  }
}

fn build_selection_system() -> ActorSystem {
  let props = Props::from_fn(|| NoopActor).with_name("selection-root");
  let config = ActorSystemConfig::new(TestTickDriver::default()).with_system_name("selection-spec");
  ActorSystem::create_with_config(&props, config).expect("system")
}

fn spawn_selection_probe(
  system: &ActorSystem,
) -> (ChildRef, ArcShared<SpinSyncMutex<Vec<String>>>, ArcShared<SpinSyncMutex<Vec<Option<Pid>>>>) {
  let messages = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let senders = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let messages = messages.clone();
    let senders = senders.clone();
    move || SelectionProbeActor::new(messages.clone(), senders.clone())
  });
  let child = system.actor_of_named(&props, "selection-target").expect("selection target");
  (child, messages, senders)
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  let deadline = Instant::now() + scaled_duration(Duration::from_secs(2));
  while Instant::now() < deadline {
    if condition() {
      return;
    }
    thread::yield_now();
  }
  assert!(condition());
}

fn test_time_factor() -> f64 {
  match env::var("TEST_TIME_FACTOR") {
    | Err(_) => 1.0,
    | Ok(raw) => {
      let factor = raw
        .parse::<f64>()
        .unwrap_or_else(|e| panic!("test_time_factor: TEST_TIME_FACTOR={raw:?} is not a valid f64: {e}"));
      assert!(factor > 0.0, "test_time_factor: TEST_TIME_FACTOR={raw:?} must be positive, got {factor}");
      factor
    },
  }
}

fn scaled_duration(base: Duration) -> Duration {
  base.mul_f64(test_time_factor())
}

#[test]
fn test_resolve_current_path() {
  // 現在のパスを維持
  let base = ActorPath::root().child("worker");
  let resolved = ActorSelectionResolver::resolve_relative(&base, ".").unwrap();
  assert_eq!(resolved.to_relative_string(), base.to_relative_string());
}

#[test]
fn test_resolve_child_path() {
  // 子パスを追加
  // ActorPath::root() は guardian "/user" を含む
  let base = ActorPath::root();
  let resolved = ActorSelectionResolver::resolve_relative(&base, "worker").unwrap();
  // 期待値は /user/worker (guardian含む)
  assert_eq!(resolved.to_relative_string(), base.child("worker").to_relative_string());
}

#[test]
fn test_resolve_multiple_child_path() {
  // 複数の子パスを追加
  let base = ActorPath::root();
  let resolved = ActorSelectionResolver::resolve_relative(&base, "worker/task").unwrap();
  let expected = base.child("worker").child("task");
  assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
}

#[test]
fn test_resolve_parent_path() {
  // 親パスへ遡る
  let base = ActorPath::root().child("worker");
  let resolved = ActorSelectionResolver::resolve_relative(&base, "..").unwrap();
  let expected = ActorPath::root();
  assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
}

#[test]
fn test_resolve_parent_and_child() {
  // 親へ遡って別の子を追加
  let base = ActorPath::root().child("worker");
  let resolved = ActorSelectionResolver::resolve_relative(&base, "../manager/ops").unwrap();
  let expected = ActorPath::root().child("manager").child("ops");
  assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
}

#[test]
fn test_escape_guardian_fails() {
  // guardian より上位へ遡ることは禁止
  let base = ActorPath::root();
  let result = ActorSelectionResolver::resolve_relative(&base, "..");
  assert!(matches!(result, Err(ActorPathError::RelativeEscape)));
}

#[test]
fn test_escape_beyond_guardian_fails() {
  // 複数の .. で guardian を超えようとする
  let base = ActorPath::root();
  let result = ActorSelectionResolver::resolve_relative(&base, "../..");
  assert!(matches!(result, Err(ActorPathError::RelativeEscape)));
}

#[test]
fn test_complex_relative_path() {
  // 複雑な相対パス解決
  let base = ActorPath::root().child("worker").child("subtask");
  let resolved = ActorSelectionResolver::resolve_relative(&base, "../../manager/newtask").unwrap();
  let expected = ActorPath::root().child("manager").child("newtask");
  assert_eq!(resolved.to_relative_string(), expected.to_relative_string());
}

#[test]
fn test_empty_selection_returns_base() {
  // 空の選択式は base をそのまま返す
  let base = ActorPath::root();
  let resolved = ActorSelectionResolver::resolve_relative(&base, "").unwrap();
  assert_eq!(resolved.to_relative_string(), base.to_relative_string());
}

// Task 3.2: Authority 未解決時の遅延配送テスト
#[test]
fn test_defer_send_when_authority_unresolved() {
  // リモート authority を持つパスを作成
  let parts = ActorPathParts::with_authority("test-system", Some(("remote-host", 2552)));
  let _remote_path = ActorPath::from_parts(parts);

  let mut registry = RemoteAuthorityRegistry::new();

  // authority が未解決なので defer_send される
  let message = AnyMessage::new(42u32);
  registry.defer_send("remote-host:2552", message).expect("defer");

  // deferred キューにメッセージが積まれていることを確認
  assert_eq!(registry.deferred_count("remote-host:2552"), 1);
}

#[test]
fn test_ensure_authority_state_defers_and_errors_when_unresolved() {
  let parts = ActorPathParts::with_authority("remote-sys", Some(("host.example.com", 2552)));
  let path = ActorPath::from_parts(parts).child("worker");
  let mut registry = RemoteAuthorityRegistry::new();
  let err =
    ActorSelectionResolver::ensure_authority_state(&path, &mut registry, Some(AnyMessage::new(1u32))).unwrap_err();
  assert!(matches!(err, PathResolutionError::AuthorityUnresolved));
  assert_eq!(registry.deferred_count("host.example.com:2552"), 1);
}

#[test]
fn test_ensure_authority_state_rejects_quarantine() {
  let parts = ActorPathParts::with_authority("remote-sys", Some(("blocked-host", 2553)));
  let path = ActorPath::from_parts(parts).child("logger");
  let mut registry = RemoteAuthorityRegistry::new();
  registry.set_quarantine("blocked-host:2553", 0, Some(Duration::from_secs(30)));

  let err =
    ActorSelectionResolver::ensure_authority_state(&path, &mut registry, Some(AnyMessage::new("msg"))).unwrap_err();
  assert!(matches!(err, PathResolutionError::AuthorityQuarantined));
  assert_eq!(registry.deferred_count("blocked-host:2553"), 0);
}

#[test]
fn test_resolve_relative_with_authority_reports_unresolved() {
  let base = ActorPath::from_parts(ActorPathParts::with_authority("cluster", Some(("peer", 2552))));
  let mut registry = RemoteAuthorityRegistry::new();
  let result = ActorSelectionResolver::resolve_relative_with_authority(
    &base,
    "worker",
    &mut registry,
    Some(AnyMessage::new("msg")),
  );
  match result {
    | Err(ActorSelectionError::Authority(PathResolutionError::AuthorityUnresolved)) => {},
    | other => panic!("unexpected result {:?}", other),
  }
  assert_eq!(registry.deferred_count("peer:2552"), 1);
}

#[test]
fn test_resolve_relative_with_authority_fails_on_quarantine() {
  let base = ActorPath::from_parts(ActorPathParts::with_authority("cluster", Some(("peer2", 2553))));
  let mut registry = RemoteAuthorityRegistry::new();
  registry.set_quarantine("peer2:2553", 0, Some(Duration::from_secs(100)));
  let result = ActorSelectionResolver::resolve_relative_with_authority(
    &base,
    "worker",
    &mut registry,
    Some(AnyMessage::new(1u32)),
  );
  assert!(matches!(result, Err(ActorSelectionError::Authority(PathResolutionError::AuthorityQuarantined))));
}

#[test]
fn test_flush_deferred_when_connected() {
  let mut registry = RemoteAuthorityRegistry::new();
  let authority = "remote-host:2552";

  // Unresolved 状態でメッセージを defer
  registry.defer_send(authority, AnyMessage::new(1u32)).expect("defer");
  registry.defer_send(authority, AnyMessage::new(2u32)).expect("defer");
  assert_eq!(registry.deferred_count(authority), 2);

  // Connected へ遷移して deferred メッセージを取得
  let deferred = registry.set_connected(authority);
  assert!(deferred.is_some());
  assert_eq!(deferred.unwrap().len(), 2);

  // キューがクリアされたことを確認
  assert_eq!(registry.deferred_count(authority), 0);
}

#[test]
fn test_reject_send_when_quarantined() {
  let mut registry = RemoteAuthorityRegistry::new();
  let authority = "quarantined-host:2552";

  // Quarantine へ遷移
  registry.set_quarantine(authority, 0, Some(Duration::from_secs(300)));

  // Quarantine 中は送信が拒否される
  let result = registry.defer_send(authority, AnyMessage::new(42u32));
  assert!(matches!(result, Err(RemoteAuthorityError::Quarantined)));
}

// Task 3.3: 統合シナリオテスト
#[test]
fn test_scenario_unresolved_to_connected_delivery() {
  let mut registry = RemoteAuthorityRegistry::new();
  let authority = "integration-host:2552";

  // シナリオ 1: 未解決状態でメッセージを積む
  registry.defer_send(authority, AnyMessage::new("msg1")).expect("defer");
  registry.defer_send(authority, AnyMessage::new("msg2")).expect("defer");
  registry.defer_send(authority, AnyMessage::new("msg3")).expect("defer");

  assert_eq!(registry.deferred_count(authority), 3);

  // シナリオ 2: 接続確立で deferred メッセージを取得
  let deferred = registry.set_connected(authority).expect("deferred queue should exist");
  assert_eq!(deferred.len(), 3);
  assert_eq!(registry.deferred_count(authority), 0);

  // シナリオ 3: 接続済みなので新規メッセージは即座に配送可能（キューに積まれない）
  // 注: 現在の実装では Connected 状態でも defer できるが、実際の remoting では即配送する
}

#[test]
fn test_scenario_multiple_relative_selections() {
  // 複数の相対パス解決を組み合わせたシナリオ
  let user = ActorPath::root();
  let worker = user.child("worker");
  let task = worker.child("task");

  // task から ../../manager/subtask へ遡る
  let resolved = ActorSelectionResolver::resolve_relative(&task, "../../manager/subtask").unwrap();
  let expected = user.child("manager").child("subtask");
  assert_eq!(resolved.to_relative_string(), expected.to_relative_string());

  // さらに ../.. で user まで戻って system/logger へ
  let resolved2 = ActorSelectionResolver::resolve_relative(&resolved, "../..").unwrap();
  let expected2 = user; // guardian "/user" まで戻る
  assert_eq!(resolved2.to_relative_string(), expected2.to_relative_string());

  // user から system/logger へ移動（guardian を超えるのではなく、sibling への移動）
  // 注: guardian (cellactor) の直下には system と user がいるため、
  // user から ../system は guardian を経由する必要がある
  // 現在の実装では guardian より上には遡れないため、このケースは失敗する
}

#[test]
fn test_scenario_guardian_boundary_protection() {
  // guardian 境界を超えようとする複数パターン
  let root = ActorPath::root();

  // パターン 1: root から ..
  assert!(matches!(ActorSelectionResolver::resolve_relative(&root, ".."), Err(ActorPathError::RelativeEscape)));

  // パターン 2: user から ../..
  let user = root.clone();
  assert!(matches!(ActorSelectionResolver::resolve_relative(&user, "../.."), Err(ActorPathError::RelativeEscape)));

  // パターン 3: 深いパスから大量の ..
  let deep = user.child("a").child("b").child("c");
  assert!(matches!(
    ActorSelectionResolver::resolve_relative(&deep, "../../../../.."),
    Err(ActorPathError::RelativeEscape)
  ));
}

#[test]
fn actor_selection_tell_delivers_to_selected_actor() {
  let system = build_selection_system();
  let (child, messages, senders) = spawn_selection_probe(&system);
  let selection = system.actor_selection(&child.actor_ref().path().expect("path").to_relative_string());

  selection.tell(AnyMessage::new(String::from("ping")), None).expect("tell");

  wait_until(|| messages.lock().len() == 1);
  assert_eq!(messages.lock().clone(), vec![String::from("ping")]);
  assert_eq!(senders.lock().clone(), vec![None]);
}

#[test]
fn actor_selection_forward_preserves_sender() {
  let system = build_selection_system();
  let (child, messages, senders) = spawn_selection_probe(&system);
  let path = child.actor_ref().path().expect("path");
  let selection = system.actor_selection_from_path(&path);
  let sender = ActorRef::new_with_builtin_lock(Pid::new(9000, 0), NullSender);

  selection.forward(AnyMessage::new(String::from("forwarded")), &sender).expect("forward");

  wait_until(|| messages.lock().len() == 1);
  assert_eq!(messages.lock().clone(), vec![String::from("forwarded")]);
  assert_eq!(senders.lock().clone(), vec![Some(sender.pid())]);
}

#[test]
fn actor_selection_resolve_one_returns_actor_identity_reply() {
  let system = build_selection_system();
  let (child, _, _) = spawn_selection_probe(&system);
  let path = child.actor_ref().path().expect("path");
  let selection = system.actor_selection_from_path(&path);

  let response = selection.resolve_one(Duration::from_millis(100)).expect("resolve_one");

  wait_until(|| response.future().with_read(|future| future.is_ready()));
  let result = response.future().with_write(|future| future.try_take()).expect("ready result");
  let reply = result.expect("identify should succeed");
  let identity = reply.downcast_ref::<ActorIdentity>().expect("ActorIdentity");

  assert_eq!(identity.actor_ref().expect("resolved actor").pid(), child.pid());
}

#[test]
fn actor_selection_to_serialization_format_returns_canonical_uri() {
  let system = build_selection_system();
  let (child, _, _) = spawn_selection_probe(&system);
  let path = child.actor_ref().path().expect("path");
  let selection = system.actor_selection_from_path(&path);

  let serialized = selection.to_serialization_format().expect("serialize");

  assert!(serialized.starts_with("fraktor://selection-spec/"));
  assert!(serialized.ends_with(&child.actor_ref().path().expect("path").to_relative_string()));
}

#[test]
fn actor_selection_tell_defers_when_remote_authority_is_unresolved() {
  let system = build_selection_system();
  let path = ActorPath::from_parts(ActorPathParts::with_authority("selection-spec", Some(("peer.example.com", 2552))))
    .child("worker");
  let selection = ActorSelection::from_path(system.state(), &path);

  let error = selection.tell(AnyMessage::new(String::from("remote")), None).expect_err("unresolved authority");

  assert!(matches!(error, ActorSelectionError::Authority(PathResolutionError::AuthorityUnresolved)));
  assert_eq!(system.state().remote_authority_deferred_count("peer.example.com:2552"), 1);
}

#[test]
fn actor_selection_forward_rejects_quarantined_authority() {
  let system = build_selection_system();
  system.state().remote_authority_set_quarantine("peer.example.com:2553", Some(Duration::from_secs(30)));
  let path = ActorPath::from_parts(ActorPathParts::with_authority("selection-spec", Some(("peer.example.com", 2553))))
    .child("worker");
  let selection = ActorSelection::from_path(system.state(), &path);
  let sender = ActorRef::new_with_builtin_lock(Pid::new(9001, 0), NullSender);

  let error =
    selection.forward(AnyMessage::new(String::from("quarantined")), &sender).expect_err("quarantined authority");

  assert!(matches!(error, ActorSelectionError::Authority(PathResolutionError::AuthorityQuarantined)));
}
