use alloc::{
  format,
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::{hint::spin_loop, num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use super::{ActorCell, actor_cell_dispatch::ActorCellInvoker};
use crate::{
  actor::{
    Actor, ActorContext, Pid, ReceiveTimeoutState, WatchRegistrationKind,
    actor_ref::dead_letter::DeadLetterReason,
    error::{ActorError, ActorErrorReason, PipeSpawnError},
    messaging::{
      ActorIdentity, AnyMessage, AnyMessageView, Identify, Kill, NotInfluenceReceiveTimeout, PoisonPill,
      message_invoker::MessageInvoker, system_message::SystemMessage,
    },
    props::{MailboxConfig, Props},
    supervision::{
      RestartLimit, SupervisorDirective, SupervisorStrategy, SupervisorStrategyConfig, SupervisorStrategyKind,
    },
  },
  dispatch::{
    dispatcher::DEFAULT_DISPATCHER_ID,
    mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  },
  system::ActorSystem,
};

struct NonInfluencingTick;

impl NotInfluenceReceiveTimeout for NonInfluencingTick {}

struct ReceiveTimeoutNoopActor;

impl Actor for ReceiveTimeoutNoopActor {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    ctx.set_receive_timeout(Duration::from_millis(20), AnyMessage::new("timeout"));
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

fn current_schedule_generation(cell: &ActorCell) -> u64 {
  cell
    .receive_timeout
    .as_shared_lock()
    .with_lock(|state| state.as_ref().map(ReceiveTimeoutState::schedule_generation))
    .expect("receive timeout should be armed")
}

struct ProbeActor;

impl Actor for ProbeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

struct RecordingActor {
  log: ArcShared<SpinSyncMutex<Vec<Pid>>>,
}

impl RecordingActor {
  fn new(log: ArcShared<SpinSyncMutex<Vec<Pid>>>) -> Self {
    Self { log }
  }
}

struct LifecycleRecorderActor {
  log: ArcShared<SpinSyncMutex<Vec<&'static str>>>,
}

impl LifecycleRecorderActor {
  fn new(log: ArcShared<SpinSyncMutex<Vec<&'static str>>>) -> Self {
    Self { log }
  }
}

impl Actor for LifecycleRecorderActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("pre_start");
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    self.log.lock().push("receive");
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("post_stop");
    Ok(())
  }
}

/// AC-H4 / AL-H1 専用のライフサイクル記録 actor。
///
/// `pre_start` / `pre_restart(reason)` / `post_stop` / `post_restart(reason)`
/// の発火順序を `String` で記録する。`pre_restart` / `post_restart` は既定実装
/// （Pekko 互換 default = stop_all_children + post_stop / pre_start 委譲）には
/// 委譲せず、`format!("pre_restart:{}", reason.as_str())` 形式で記録する。
/// これにより「fault_recreate がいつ deferred されているか」「reason payload が
/// 失われずに post_restart へ届いているか」を観測できる。
struct RestartLifecycleRecorderActor {
  log: ArcShared<SpinSyncMutex<Vec<String>>>,
}

impl RestartLifecycleRecorderActor {
  fn new(log: ArcShared<SpinSyncMutex<Vec<String>>>) -> Self {
    Self { log }
  }
}

impl Actor for RestartLifecycleRecorderActor {
  fn pre_start(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("pre_start".to_string());
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn post_stop(&mut self, _ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    self.log.lock().push("post_stop".to_string());
    Ok(())
  }

  fn pre_restart(&mut self, _ctx: &mut ActorContext<'_>, reason: &ActorErrorReason) -> Result<(), ActorError> {
    // AL-H1 で追加される `reason: &ActorErrorReason` 引数を観測する。既定実装に
    // 委譲しないことで「kernel 側が pre_restart を 1 回だけ呼ぶ」契約を確認する。
    self.log.lock().push(format!("pre_restart:{}", reason.as_str()));
    Ok(())
  }

  fn post_restart(&mut self, _ctx: &mut ActorContext<'_>, reason: &ActorErrorReason) -> Result<(), ActorError> {
    // AL-H1 で追加される `post_restart`。既定では `pre_start` を呼ぶ Pekko
    // 互換 default を持つが、本テストでは override で reason payload を記録し、
    // 既定の pre_start 委譲を行わない（pre_start は kernel 側が別途駆動する）。
    self.log.lock().push(format!("post_restart:{}", reason.as_str()));
    Ok(())
  }
}

impl Actor for RecordingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn on_terminated(&mut self, _ctx: &mut ActorContext<'_>, pid: Pid) -> Result<(), ActorError> {
    self.log.lock().push(pid);
    Ok(())
  }
}

struct OrderedMessageActor {
  received: ArcShared<SpinSyncMutex<Vec<i32>>>,
}

impl OrderedMessageActor {
  fn new(received: ArcShared<SpinSyncMutex<Vec<i32>>>) -> Self {
    Self { received }
  }
}

impl Actor for OrderedMessageActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if let Some(value) = message.downcast_ref::<i32>() {
      self.received.lock().push(*value);
    }
    Ok(())
  }
}

struct IdentityProbeActor {
  received: ArcShared<SpinSyncMutex<usize>>,
  replies:  ArcShared<SpinSyncMutex<Vec<ActorIdentity>>>,
}

impl IdentityProbeActor {
  fn new(received: ArcShared<SpinSyncMutex<usize>>, replies: ArcShared<SpinSyncMutex<Vec<ActorIdentity>>>) -> Self {
    Self { received, replies }
  }
}

impl Actor for IdentityProbeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    *self.received.lock() += 1;
    if let Some(identity) = message.downcast_ref::<ActorIdentity>() {
      self.replies.lock().push(identity.clone());
    }
    Ok(())
  }
}

struct ReceiveTimeoutFailingActor;

impl Actor for ReceiveTimeoutFailingActor {
  fn pre_start(&mut self, ctx: &mut ActorContext<'_>) -> Result<(), ActorError> {
    ctx.set_receive_timeout(Duration::from_millis(20), AnyMessage::new("timeout"));
    Ok(())
  }

  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<u32>().is_some() {
      return Err(ActorError::recoverable("boom"));
    }
    Ok(())
  }
}

struct ResumeSupervisorActor;

impl Actor for ResumeSupervisorActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn supervisor_strategy(&self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    SupervisorStrategy::new(
      SupervisorStrategyKind::OneForOne,
      RestartLimit::WithinWindow(1),
      Duration::from_secs(1),
      |_| SupervisorDirective::Resume,
    )
    .into()
  }
}

// NOTE: previously `register_watch_with_replaces_previous_entry_for_same_target`
// verified the silent-overwrite behaviour. After change
// `pekko-death-watch-duplicate-check` (Decision 4), `register_watch_with` is a
// `pub(crate)` internal whose invariant is "upstream `watch_registration_kind`
// check has already validated there is no existing entry". The silent
// overwrite is therefore no longer part of the contract (debug builds panic
// via `debug_assert!`), and the context-level duplicate detection is covered
// by `actor_context::tests::watch_with_after_watch_with_always_rejects`.

/// AC-H3-T4 専用の失敗生成 actor。u32 メッセージを受けると recoverable 失敗を
/// 返し、`ActorCellInvoker::invoke` の Err 経路経由で `report_failure` を発火する。
struct FailingOnU32Actor;

impl Actor for FailingOnU32Actor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, message: AnyMessageView<'_>) -> Result<(), ActorError> {
    if message.downcast_ref::<u32>().is_some() {
      return Err(ActorError::recoverable("ac-h3-t4-boom"));
    }
    Ok(())
  }
}

// ============================================================================
// AC-H3 拡張: FailedInfo state (PIDs 250-259)
//
// Pekko `FaultHandling.scala` の `_failed: FailedInfo` 状態 (NoFailedInfo /
// FailedRef(perpetrator) / FailedFatally) を ActorCell の public(crate) API
// (`is_failed` / `set_failed` / `clear_failed` / `perpetrator` /
// `is_failed_fatally` / `set_failed_fatally`) として観測する。これらの
// accessor は AC-H3 拡張で新設される forward-looking API である。
// ============================================================================

// ============================================================================
// AC-M3: `report_failure` wires `set_failed(self.pid)` with `is_failed()` guard
// (Pekko `FaultHandling.scala:218-234` handleInvokeFailure parity).
// ============================================================================

// ============================================================================
// AC-H2: ChildrenContainer 4-state machine production wiring (PIDs 270-279)
//
// `set_children_termination_reason` / `is_normal` / `is_terminating` は
// ChildrenContainer 層に存在するが、ActorCell production paths からはまだ
// 呼ばれていない。AC-H2 はこれらを fault_terminate / fault_recreate 経路に
// 接続し、4-state machine (Empty/Normal/Terminating/Terminated) を完成させる。
// 観測は `cell.children_state_is_normal()` / `children_state_is_terminating()`
// および lifecycle log の遅延として行う。
// ============================================================================

// ============================================================================
// AC-H4: fault_recreate / finish_* completion waiting (PIDs 300-339)
//
// Pekko `FaultHandling.scala:215-237` `faultRecreate` のフロー:
//   1. isFailedFatally なら no-op
//   2. pre_restart(cause) を 1 回だけ呼ぶ
//   3. childrenRefs.isNormal なら finishRecreate(cause) を即座に実行
//   4. そうでなければ ChildrenContainer を Recreation(cause) で suspend し、 最後の子が
//      handle_terminated されたタイミングで finishRecreate を遅延実行
//
// finishRecreate(cause):
//   - reset _failed
//   - recreate_actor + post_restart(cause) (pre_start は post_restart 既定で委譲)
//   - mailbox.resume
//
// SystemMessage::Recreate(ActorErrorReason) ペイロードを通じて cause が
// 失われずに pre_restart / post_restart へ届く必要がある。
// ============================================================================

// ============================================================================
// AL-H1: post_restart hook + pre_restart Pekko-compliant default (PIDs 800-899)
//
// Pekko `Actor.scala` の preRestart / postRestart 既定実装:
//   def preRestart(reason: Throwable, message: Option[Any]): Unit = {
//     context.children foreach { child =>
//       context.unwatch(child)
//       context.stop(child)
//     }
//     postStop()
//   }
//   def postRestart(reason: Throwable): Unit = preStart()
//
// 検証する契約:
//   - 既定 pre_restart は stop_all_children + post_stop を呼ぶ
//   - 既定 post_restart は pre_start を呼ぶ
//   - kernel 側は restart 経路で pre_start を自動的に呼ばない（post_restart 既定が委譲）
//   - Override は default を完全に置き換える（kernel が再委譲しない）
// ============================================================================

// ============================================================================
// AC-H5: terminatedQueued + DeathWatch user-queue delivery (PIDs 500-599)
//
// Pekko `DeathWatch.scala`:
//   - `watching: HashSet[ActorRef]` ── 自分が watch している相手の集合
//   - `terminatedQueued: HashSet[ActorRef]` ── DeathWatchNotification を受けた後、 user queue に
//     Terminated を投入済み (= 重複投入を抑止) のマーカー
//   - `watchedActorTerminated(actor)` ── DeathWatchNotification ハンドラ: if
//     (watching.contains(actor) && !isTerminating) self.tell(Terminated(actor)); terminatedQueued
//     += actor
//
// fraktor-rs では:
//   - `SystemMessage::DeathWatchNotification(Pid)` を kernel 内通知に使う
//   - watcher 側は `state.watching` (新設) と `state.terminated_queued` (新設) で dedup
//     し、user-level `Terminated` を user queue へ投入する
//   - 既存の `SystemMessage::Terminated(Pid)` は user-level セマンティクスへ寄せる
// ============================================================================

// === AC-M4a: watch_registration_kind query ============================
//
// Pekko `DeathWatch.scala:104` `watching.get(actor)` の 3 値セマンティクスを
// fraktor-rs の split data structure (`watching` + `watch_with_messages`) と
// `WatchKind::User` フィルタで合成できることを検証する。

#[path = "actor_cell_adapter_handles_test.rs"]
mod adapter_handles;
#[path = "actor_cell_children_test.rs"]
mod children;
#[path = "actor_cell_death_watch_test.rs"]
mod death_watch;
#[path = "actor_cell_dispatch_test.rs"]
mod dispatch;
#[path = "actor_cell_fault_handling_test.rs"]
mod fault_handling;
#[path = "actor_cell_lifecycle_test.rs"]
mod lifecycle;
#[path = "actor_cell_pipe_tasks_test.rs"]
mod pipe_tasks;
#[path = "actor_cell_receive_timeout_test.rs"]
mod receive_timeout;
#[path = "actor_cell_stash_test.rs"]
mod stash;
#[path = "actor_cell_timers_test.rs"]
mod timers;

#[test]
fn actor_cell_holds_components() {
  let system = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(system, Pid::new(1, 0), None, "worker".to_string(), &props).expect("create actor cell");

  assert_eq!(cell.pid(), Pid::new(1, 0));
  assert_eq!(cell.name(), "worker");
  assert!(cell.parent().is_none());
  assert_eq!(cell.mailbox().system_len(), 0);
}

#[test]
fn actor_cell_scheduler_accessor_returns_system_scheduler() {
  let actor_system = ActorSystem::new_empty();
  let system = actor_system.state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(system, Pid::new(901, 0), None, "scheduler".to_string(), &props).expect("cell");

  let system_scheduler = actor_system.scheduler();
  assert!(!system_scheduler.with_read(|scheduler| scheduler.diagnostics().is_log_enabled()));

  cell.scheduler().with_write(|scheduler| scheduler.enable_deterministic_log(4));

  assert!(cell.scheduler().with_read(|scheduler| scheduler.diagnostics().is_log_enabled()));
  assert!(system_scheduler.with_read(|scheduler| scheduler.diagnostics().is_log_enabled()));
}

#[test]
fn actor_cell_create_same_as_parent_without_parent_uses_default_dispatcher() {
  let actor_system = ActorSystem::new_empty();
  let system = actor_system.state();
  let props = Props::from_fn(|| ProbeActor).with_dispatcher_same_as_parent();
  let cell = ActorCell::create(system, Pid::new(902, 0), None, "root-child".to_string(), &props).expect("cell");

  assert_eq!(cell.dispatcher_id(), DEFAULT_DISPATCHER_ID);
}

#[test]
fn actor_cell_create_with_mailbox_id_uses_registered_mailbox_policy() {
  // The registered "bounded" policy has capacity 1 with DropNewest semantics,
  // so the second user enqueue should be rejected even though `Props` requests
  // an unbounded mismatched policy.
  let registered_policy = MailboxPolicy::bounded(
    NonZeroUsize::new(1).expect("non-zero mailbox capacity"),
    MailboxOverflowStrategy::DropNewest,
    None,
  );
  let system =
    ActorSystem::new_empty_with(|config| config.with_mailbox("bounded", MailboxConfig::new(registered_policy))).state();

  let mismatched_policy = MailboxPolicy::unbounded(None);
  let props =
    Props::from_fn(|| ProbeActor).with_mailbox_config(MailboxConfig::new(mismatched_policy)).with_mailbox_id("bounded");

  let cell = ActorCell::create(system, Pid::new(2, 0), None, "worker".to_string(), &props).expect("create actor cell");

  let mailbox = cell.mailbox();
  mailbox.enqueue_user(AnyMessage::new(1_u32)).expect("first enqueue fits the bounded capacity");
  // DropNewest overflow は mailbox 層で DeadLetters へ転送され、Pekko の
  // void-on-success 契約として成功扱いになる。queue は capacity 1 のままなので、
  // Props の unbounded 設定ではなく登録済み bounded policy が有効であることを
  // 検証できる。
  mailbox
    .enqueue_user(AnyMessage::new(2_u32))
    .expect("DropNewest overflow reports success after routing to DeadLetters");
  assert_eq!(mailbox.user_len(), 1, "registered bounded mailbox must reject the second enqueue past capacity 1");
}

#[test]
fn actor_cell_mailbox_accessor_returns_stable_shared_handle() {
  let system = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(system, Pid::new(701, 0), None, "mailbox-slot".to_string(), &props).expect("cell");

  let first = cell.mailbox();
  let second = cell.mailbox();
  assert!(ArcShared::ptr_eq(&first, &second));
}

#[test]
fn tags_propagated_from_props_to_cell() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor).with_tags(["metrics", "routing"]);
  let cell = ActorCell::create(state.clone(), Pid::new(90, 0), None, "tagged".to_string(), &props).expect("create");
  state.register_cell(cell.clone());

  let tags = cell.tags();
  assert_eq!(tags.len(), 2);
  assert!(tags.contains("metrics"));
  assert!(tags.contains("routing"));
}

#[test]
fn tags_empty_when_props_has_no_tags() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(91, 0), None, "untagged".to_string(), &props).expect("create");
  state.register_cell(cell.clone());

  assert!(cell.tags().is_empty());
}

fn wait_until(mut condition: impl FnMut() -> bool) {
  for _ in 0..10_000 {
    if condition() {
      return;
    }
    spin_loop();
  }
  assert!(condition());
}
