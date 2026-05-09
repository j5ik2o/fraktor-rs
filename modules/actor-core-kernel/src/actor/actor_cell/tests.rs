use alloc::{
  format,
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::{hint::spin_loop, num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SpinSyncMutex};

use super::{ActorCell, ActorCellInvoker};
use crate::{
  actor::{
    Actor, ActorContext, Pid, ReceiveTimeoutState, WatchRegistrationKind,
    error::{ActorError, ActorErrorReason},
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
fn actor_cell_stop_child_ignores_unknown_child_pid() {
  let actor_system = ActorSystem::new_empty();
  let system = actor_system.state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(system, Pid::new(903, 0), None, "parent".to_string(), &props).expect("cell");
  let known_child = Pid::new(905, 0);
  cell.register_child(known_child);

  cell.stop_child(Pid::new(904, 0));

  assert_eq!(cell.children(), vec![known_child]);
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
fn handle_watch_is_idempotent() {
  let system = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let target =
    ActorCell::create(system.clone(), Pid::new(10, 0), None, "target".to_string(), &props).expect("create actor cell");
  system.register_cell(target.clone());

  target.handle_watch(Pid::new(20, 0));
  target.handle_watch(Pid::new(20, 0));

  assert_eq!(target.watchers_snapshot().len(), 1);
}

#[test]
fn handle_unwatch_removes_pid() {
  let system = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let target =
    ActorCell::create(system.clone(), Pid::new(11, 0), None, "target".to_string(), &props).expect("create actor cell");
  system.register_cell(target.clone());

  target.handle_watch(Pid::new(21, 0));
  target.handle_unwatch(Pid::new(21, 0));

  assert_eq!(target.watchers_snapshot().len(), 0);
}

#[test]
fn notify_watchers_sends_terminated() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let target =
    ActorCell::create(state.clone(), Pid::new(30, 0), None, "target".to_string(), &props).expect("create actor cell");
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let watcher_props = Props::from_fn({
    let log = log.clone();
    move || RecordingActor::new(log.clone())
  });
  let watcher = ActorCell::create(state.clone(), Pid::new(31, 0), None, "watcher".to_string(), &watcher_props)
    .expect("create actor cell");
  state.register_cell(target.clone());
  state.register_cell(watcher.clone());

  // AC-H5: watcher 側でも target を watching に登録しておかないと、
  // DeathWatchNotification 受信時に `watching_contains_pid` 判定で dropped される。
  watcher.register_watching(target.pid());
  target.handle_watch(watcher.pid());
  target.notify_watchers_on_stop();
  assert_eq!(log.lock().clone(), vec![target.pid()]);
  assert_eq!(target.watchers_snapshot().len(), 0);
}

#[test]
fn drop_adapter_refs_marks_lifecycle_stopped() {
  let system = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(system.clone(), Pid::new(50, 0), None, "adapter".to_string(), &props).expect("create actor cell");
  system.register_cell(cell.clone());

  let (_id, lifecycle) = cell.acquire_adapter_handle();
  assert!(lifecycle.is_alive());

  cell.drop_adapter_refs();
  assert!(!lifecycle.is_alive());
}

#[test]
fn remove_adapter_handle_stops_single_handle() {
  let system = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(system.clone(), Pid::new(51, 0), None, "adapter".to_string(), &props).expect("create actor cell");
  system.register_cell(cell.clone());

  let (id, lifecycle) = cell.acquire_adapter_handle();
  assert!(lifecycle.is_alive());

  cell.remove_adapter_handle(id);
  assert!(!lifecycle.is_alive());
}

#[test]
fn create_system_message_runs_pre_start() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(40, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  let snapshot = log.lock().clone();
  assert_eq!(snapshot, vec!["pre_start"]);
}

#[test]
fn identify_replies_with_actor_identity_without_invoking_actor() {
  let system = ActorSystem::new_empty().state();
  let actor_received = ArcShared::new(SpinSyncMutex::new(0usize));
  let actor_replies = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let actor_props = Props::from_fn({
    let actor_received = actor_received.clone();
    let actor_replies = actor_replies.clone();
    move || IdentityProbeActor::new(actor_received.clone(), actor_replies.clone())
  });
  let target =
    ActorCell::create(system.clone(), Pid::new(60, 0), None, "target".to_string(), &actor_props).expect("target");
  let reply_received = ArcShared::new(SpinSyncMutex::new(0usize));
  let reply_replies = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let reply_props = Props::from_fn({
    let reply_received = reply_received.clone();
    let reply_replies = reply_replies.clone();
    move || IdentityProbeActor::new(reply_received.clone(), reply_replies.clone())
  });
  let reply_to =
    ActorCell::create(system.clone(), Pid::new(61, 0), None, "reply".to_string(), &reply_props).expect("reply");
  system.register_cell(target.clone());
  system.register_cell(reply_to.clone());

  let mut invoker = ActorCellInvoker { cell: target.downgrade() };
  let identify = Identify::new(AnyMessage::new("corr"));
  let message = AnyMessage::new(identify).with_sender(reply_to.actor_ref());

  invoker.invoke(message).expect("identify");

  assert_eq!(*actor_received.lock(), 0, "identify should not reach the actor receive method");
  wait_until(|| reply_replies.lock().len() == 1);
  let replies = reply_replies.lock();
  assert_eq!(replies.len(), 1);
  let correlation_id = replies[0].correlation_id().payload().downcast_ref::<&str>().expect("&str");
  assert_eq!(*correlation_id, "corr");
  assert_eq!(replies[0].actor_ref().expect("actor ref").pid(), target.pid());
}

#[test]
fn recreate_system_message_invokes_post_stop_then_pre_start() {
  // AC-H4: Pekko `faultRecreate` は mailbox が既に suspended であることを
  // 前提としているため、`report_failure` 経路を経由しない本テストでは明示的に
  // `mailbox().suspend()` を呼んで前提を整える。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(41, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  cell.mailbox().suspend();
  let cause = ActorErrorReason::new("recreate-test-cause");
  invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate");

  let snapshot = log.lock().clone();
  assert_eq!(snapshot, vec!["pre_start", "post_stop", "pre_start"]);
}

#[test]
fn poison_pill_system_message_invokes_post_stop() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(410, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  invoker.system_invoke(SystemMessage::PoisonPill).expect("poison pill");

  let snapshot = log.lock().clone();
  assert_eq!(snapshot, vec!["pre_start", "post_stop"]);
}

#[test]
fn poison_pill_public_message_invokes_post_stop() {
  // Given: 起動済み actor に public PoisonPill payload を直接送る
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(420, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  // When: public message を通常 user message 経路で配送する
  invoker.invoke(AnyMessage::new(PoisonPill)).expect("poison pill");

  // Then: SystemMessage alias ではなくても auto-receive として停止処理が走る
  let snapshot = log.lock().clone();
  assert_eq!(snapshot, vec!["pre_start", "post_stop"]);
}

#[test]
fn kill_system_message_reports_fatal_failure() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(411, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  let error = invoker.system_invoke(SystemMessage::Kill).expect_err("kill should report failure");

  assert_eq!(error, ActorError::fatal("Kill"));
}

#[test]
fn kill_public_message_reports_fatal_failure() {
  // Given: 起動済み actor に public Kill payload を直接送る
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(421, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  // When: public message を通常 user message 経路で配送する
  let error = invoker.invoke(AnyMessage::new(Kill)).expect_err("kill should fail");

  // Then: runtime は public payload を fatal kill として扱う
  assert_eq!(error, ActorError::fatal("Kill"));
}

#[test]
fn poison_pill_user_message_preserves_user_ordering() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(412, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  assert!(cell.actor_ref().try_tell(AnyMessage::new(1_u8)).is_ok());
  cell.actor_ref().poison_pill();
  // Message 2 races against the dispatcher processing PoisonPill. It is
  // either accepted (and later drained at close) or rejected with
  // `SendError::Closed` if the mailbox has already been closed. Either way,
  // the ordering invariant below requires that it is not received.
  let second_result = cell.actor_ref().try_tell(AnyMessage::new(2_u8));
  assert!(
    second_result.is_ok() || matches!(second_result, Err(crate::actor::error::SendError::Closed(_))),
    "message 2 should be accepted or rejected as Closed, got {second_result:?}",
  );

  wait_until(|| log.lock().len() >= 3);
  let snapshot = log.lock().clone();
  assert_eq!(snapshot, vec!["pre_start", "receive", "post_stop"]);
}

#[test]
fn kill_user_message_reports_fatal_failure() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(413, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  let error = invoker.invoke(AnyMessage::new(SystemMessage::Kill)).expect_err("kill should fail");
  assert_eq!(error, ActorError::fatal("Kill"));
}

#[test]
fn user_message_failure_does_not_reschedule_receive_timeout() {
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ResumeSupervisorActor);
  let parent = ActorCell::create(state.clone(), Pid::new(414, 0), None, "parent".to_string(), &parent_props)
    .expect("create parent");
  let props = Props::from_fn(|| ReceiveTimeoutFailingActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(415, 0), Some(parent.pid()), "timeout-failure".to_string(), &props)
      .expect("create actor cell");
  state.register_cell(parent.clone());
  state.register_cell(cell.clone());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("create parent");

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  let initial_handle = cell
    .receive_timeout
    .as_shared_lock()
    .with_lock(|state| state.as_ref().and_then(ReceiveTimeoutState::handle_raw))
    .expect("receive timeout handle should exist after pre_start");

  let error = invoker.invoke(AnyMessage::new(1_u32)).expect_err("user message should fail");
  assert_eq!(error, ActorError::recoverable("boom"));

  let current_handle = cell
    .receive_timeout
    .as_shared_lock()
    .with_lock(|state| state.as_ref().and_then(ReceiveTimeoutState::handle_raw))
    .expect("receive timeout handle should remain registered after failure");

  assert_eq!(current_handle, initial_handle, "failure path must not arm a fresh receive-timeout timer");
}

#[test]
fn not_influence_message_skips_reschedule() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ReceiveTimeoutNoopActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(416, 0), None, "timeout-skip".to_string(), &props).expect("create cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  let gen_before = current_schedule_generation(&cell);
  invoker.invoke(AnyMessage::not_influence(NonInfluencingTick)).expect("invoke");
  let gen_after = current_schedule_generation(&cell);

  assert_eq!(gen_after, gen_before, "NotInfluenceReceiveTimeout payload must skip reschedule");
}

#[test]
fn regular_message_reschedules_receive_timeout() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ReceiveTimeoutNoopActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(417, 0), None, "timeout-reset".to_string(), &props).expect("create cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  let gen_before = current_schedule_generation(&cell);
  invoker.invoke(AnyMessage::new(NonInfluencingTick)).expect("invoke");
  let gen_after = current_schedule_generation(&cell);

  assert_eq!(gen_after, gen_before + 1, "regular payload must cancel and reschedule (one extra schedule call)");
}

#[test]
fn system_queue_is_drained_before_user_queue() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(42, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  cell.new_dispatcher_shared().system_dispatch(&cell, SystemMessage::Create).expect("system enqueue");
  assert!(cell.actor_ref().try_tell(AnyMessage::new(())).is_ok());

  let _scheduled = cell.new_dispatcher_shared().register_for_execution(&cell.mailbox(), true, true);

  let snapshot = log.lock().clone();
  assert_eq!(snapshot, vec!["pre_start", "receive"]);
}

#[test]
fn unstash_messages_are_replayed_before_existing_mailbox_messages() {
  let state = ActorSystem::new_empty().state();
  let received = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let captured = received.clone();
    move || OrderedMessageActor::new(captured.clone())
  })
  .with_stash_mailbox();
  let cell =
    ActorCell::create(state.clone(), Pid::new(60, 0), None, "ordered".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  cell.new_dispatcher_shared().system_dispatch(&cell, SystemMessage::Create).expect("create");
  cell.stash_message_with_limit(AnyMessage::new(1_i32), usize::MAX).expect("stashing below limit should succeed");
  cell.mailbox().enqueue_user(AnyMessage::new(2_i32)).expect("enqueue queued");

  let unstashed = cell.unstash_messages().expect("unstash");
  assert_eq!(unstashed, 1);

  wait_until(|| received.lock().len() == 2);
  assert_eq!(received.lock().clone(), vec![1, 2]);
}

#[test]
fn stash_message_with_limit_rejects_non_deque_mailbox_without_buffering() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(61, 0), None, "stash-reject".to_string(), &props)
    .expect("create actor cell");

  let error =
    cell.stash_message_with_limit(AnyMessage::new(1_i32), usize::MAX).expect_err("non-deque stash should fail");

  assert!(ActorContext::is_stash_requires_deque_error(&error));
  assert_eq!(cell.stashed_message_len(), 0);
}

#[test]
fn unstash_message_rejects_non_deque_mailbox_without_consuming_stash() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(62, 0), None, "unstash-reject".to_string(), &props)
    .expect("create actor cell");

  cell.state.with_write(|state| state.stashed_messages.push_back(AnyMessage::new(1_i32)));

  let error = cell.unstash_message().expect_err("non-deque unstash should fail");

  assert!(ActorContext::is_stash_requires_deque_error(&error));
  assert_eq!(cell.stashed_message_len(), 1);
}

#[test]
fn unstash_messages_reject_non_deque_mailbox_without_consuming_stash() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(63, 0), None, "unstash-all-reject".to_string(), &props)
    .expect("create actor cell");

  cell.state.with_write(|state| {
    state.stashed_messages.push_back(AnyMessage::new(1_i32));
    state.stashed_messages.push_back(AnyMessage::new(2_i32));
  });

  let all_error = cell.unstash_messages().expect_err("non-deque unstash should fail");
  assert!(ActorContext::is_stash_requires_deque_error(&all_error));
  assert_eq!(cell.stashed_message_len(), 2);

  let limited_error = cell.unstash_messages_with_limit(1, Ok).expect_err("non-deque unstash with limit should fail");
  assert!(ActorContext::is_stash_requires_deque_error(&limited_error));
  assert_eq!(cell.stashed_message_len(), 2);
}

#[test]
fn empty_unstash_is_noop_even_without_deque_mailbox() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(64, 0), None, "unstash-empty".to_string(), &props)
    .expect("create actor cell");

  assert_eq!(cell.unstash_message().expect("empty unstash single"), 0);
  assert_eq!(cell.unstash_messages().expect("empty unstash all"), 0);
  assert_eq!(cell.unstash_messages_with_limit(1, Ok).expect("empty unstash limit"), 0);
}

#[test]
fn register_watch_with_stores_and_take_returns_message() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(70, 0), None, "watcher".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let target_pid = Pid::new(71, 0);
  cell.register_watch_with(target_pid, AnyMessage::new(42_i32));

  assert!(cell.take_watch_with_message(target_pid).is_some());
  assert!(cell.take_watch_with_message(target_pid).is_none());
}

#[test]
fn remove_watch_with_clears_custom_message() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(72, 0), None, "watcher".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let target_pid = Pid::new(73, 0);
  cell.register_watch_with(target_pid, AnyMessage::new(42_i32));
  cell.remove_watch_with(target_pid);

  assert!(cell.take_watch_with_message(target_pid).is_none());
}

// NOTE: previously `register_watch_with_replaces_previous_entry_for_same_target`
// verified the silent-overwrite behaviour. After change
// `pekko-death-watch-duplicate-check` (Decision 4), `register_watch_with` is a
// `pub(crate)` internal whose invariant is "upstream `watch_registration_kind`
// check has already validated there is no existing entry". The silent
// overwrite is therefore no longer part of the contract (debug builds panic
// via `debug_assert!`), and the context-level duplicate detection is covered
// by `actor_context::tests::watch_with_after_watch_with_always_rejects`.

#[test]
fn handle_death_watch_notification_skips_on_terminated_when_watch_with_registered() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let watcher_props = Props::from_fn({
    let log = log.clone();
    move || RecordingActor::new(log.clone())
  });
  let watcher = ActorCell::create(state.clone(), Pid::new(80, 0), None, "watcher".to_string(), &watcher_props)
    .expect("create watcher");
  state.register_cell(watcher.clone());

  let target_pid = Pid::new(81, 0);
  watcher.register_watch_with(target_pid, AnyMessage::new(42_i32));
  watcher.register_watching(target_pid);
  let result = watcher.handle_death_watch_notification(target_pid);
  assert!(result.is_ok());
  assert!(log.lock().is_empty(), "on_terminated should not be called when watch_with is registered");
}

#[test]
fn handle_death_watch_notification_removes_child_from_children() {
  // AC-H4 統合テスト: `handle_death_watch_notification` の内部で
  // `remove_child_and_get_state_change` が呼ばれ、子が children() から取り除かれる
  // ことを、kernel 層の ChildrenContainer state machine 経由で検証する。
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(100, 0), None, "parent".to_string(), &parent_props)
    .expect("create parent");
  state.register_cell(parent.clone());

  let child_pid = Pid::new(101, 0);
  parent.register_child(child_pid);
  parent.register_watching(child_pid);
  assert_eq!(parent.children(), vec![child_pid]);

  parent.handle_death_watch_notification(child_pid).expect("handle_death_watch_notification should succeed");

  assert!(parent.children().is_empty(), "children() は handle_death_watch_notification 後に空になる必要がある");
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

#[test]
fn ac_h3_t1_parent_suspend_propagates_to_child_mailbox() {
  // AC-H3-T1: 親 cell に SystemMessage::Suspend を投げると、登録済み子 cell の
  // mailbox が suspended 状態に遷移する。Pekko `FaultHandling.scala:124-128`
  // (`faultSuspend` → `suspendChildren`) の契約を kernel 層で検証する。
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(200, 0), None, "parent".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child = ActorCell::create(state.clone(), Pid::new(201, 0), Some(parent.pid()), "child".to_string(), &child_props)
    .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());

  assert!(!child.mailbox().is_suspended(), "pre-condition: 子は未 suspend で始まる");

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Suspend).expect("parent suspend");

  // 子 mailbox に届いた Suspend を同期的に drain する（テスト用 executor は
  // 明示 register で inline 実行される）。
  let _scheduled = child.new_dispatcher_shared().register_for_execution(&child.mailbox(), false, true);

  assert!(child.mailbox().is_suspended(), "AC-H3: 親 Suspend 後、子 mailbox は suspended に遷移していなければならない");
}

#[test]
fn ac_h3_t2_parent_resume_propagates_to_child_mailbox() {
  // AC-H3-T2: 親 cell に SystemMessage::Resume を投げると、事前に suspended
  // していた子 mailbox の suspend count がデクリメントされ、稼働可能状態に戻る。
  // Pekko `FaultHandling.scala:136-153` (`faultResume` → `resumeChildren`)。
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(210, 0), None, "parent".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child = ActorCell::create(state.clone(), Pid::new(211, 0), Some(parent.pid()), "child".to_string(), &child_props)
    .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());

  // 事前条件: 子 mailbox を 1 回 suspend してから親 Resume を発火する。
  // mailbox.suspend は pub(crate) なので同一クレート内テストから直接呼べる。
  child.mailbox().suspend();
  assert!(child.mailbox().is_suspended(), "pre-condition: 子は事前 suspend 済みである");

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Resume).expect("parent resume");

  let _scheduled = child.new_dispatcher_shared().register_for_execution(&child.mailbox(), false, true);

  assert!(
    !child.mailbox().is_suspended(),
    "AC-H3: 親 Resume 後、子 mailbox の suspend count は 0 に戻っていなければならない"
  );
}

#[test]
fn ac_h3_t3_suspend_propagates_recursively_to_grandchild() {
  // AC-H3-T3: 親に SystemMessage::Suspend を投げると、子→孫の 2 段で再帰的に
  // Suspend が伝播する。Pekko `Children.scala:203-208` の
  // `childrenRefs.stats.foreach { child.suspend() }` が各子 cell の
  // `process_all_system_messages` を経由して自身の子孫へさらに展開される
  // ことを確認する。
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(220, 0), None, "parent".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child = ActorCell::create(state.clone(), Pid::new(221, 0), Some(parent.pid()), "child".to_string(), &child_props)
    .expect("create child");
  let grandchild_props = Props::from_fn(|| ProbeActor);
  let grandchild =
    ActorCell::create(state.clone(), Pid::new(222, 0), Some(child.pid()), "grandchild".to_string(), &grandchild_props)
      .expect("create grandchild");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  state.register_cell(grandchild.clone());
  parent.register_child(child.pid());
  child.register_child(grandchild.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Suspend).expect("parent suspend");

  // 子 → 孫 の 2 段階 drain を明示的に発火する。各 register_for_execution は
  // 1 段分の `process_all_system_messages` を駆動し、それが further な
  // `send_system_message` を経由して孫側のキューに届いた Suspend を
  // drain する次の register_for_execution で確定する。
  let _child_scheduled = child.new_dispatcher_shared().register_for_execution(&child.mailbox(), false, true);
  let _grandchild_scheduled =
    grandchild.new_dispatcher_shared().register_for_execution(&grandchild.mailbox(), false, true);

  assert!(child.mailbox().is_suspended(), "AC-H3: 第 1 段 (子) は親 Suspend 後に suspended になっていなければならない");
  assert!(
    grandchild.mailbox().is_suspended(),
    "AC-H3: 第 2 段 (孫) も子 Suspend の再帰伝播で suspended になっていなければならない"
  );
}

#[test]
fn ac_h3_t4_report_failure_suspends_children_before_reporting() {
  // AC-H3-T4: 親の user message 処理で recoverable 失敗が発生したとき、
  // `report_failure` が親の mailbox を suspend するのと同時に、登録済みの
  // 子 mailbox にも Suspend を再帰伝播する。Pekko `FaultHandling.scala:62-67`
  // (`handleInvokeFailure`) が `suspendNonRecursive` に続いて
  // `suspendChildren` を呼ぶ契約を kernel 層に合わせて検証する。
  //
  // ここでは `invoker.invoke(failing_message)` が Err を返した時点で
  // `report_failure` が既に完了している事実を利用し、「Failure 報告後の
  // 観測時点で子が既に suspended」であることを assert する。これにより
  // `system.report_failure` へのペイロード送出より前に子 Suspend が
  // 完了しているという時系列契約を間接的に保証する。
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| FailingOnU32Actor);
  let parent = ActorCell::create(state.clone(), Pid::new(230, 0), None, "parent".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child = ActorCell::create(state.clone(), Pid::new(231, 0), Some(parent.pid()), "child".to_string(), &child_props)
    .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");

  assert!(!child.mailbox().is_suspended(), "pre-condition: 子は未 suspend である");
  assert!(!parent.mailbox().is_suspended(), "pre-condition: 親は未 suspend である");

  let error = parent_invoker.invoke(AnyMessage::new(1_u32)).expect_err("failing user message must surface Err");
  assert_eq!(error, ActorError::recoverable("ac-h3-t4-boom"));

  // report_failure は invoker.invoke 内で同期的に呼ばれるため、この時点で
  // 親自身の mailbox も suspended になっている（MB-H1 の既存契約）。
  assert!(parent.mailbox().is_suspended(), "report_failure は親 mailbox を suspend しなければならない (既存契約)");

  // 子 mailbox に配送された Suspend は process_all_system_messages 経由で
  // 反映されるため、明示 drain を発火して AC-H3 の新契約を確定する。
  let _scheduled = child.new_dispatcher_shared().register_for_execution(&child.mailbox(), false, true);

  assert!(
    child.mailbox().is_suspended(),
    "AC-H3: report_failure 経路でも子 mailbox は suspended に遷移していなければならない"
  );
}

#[test]
fn ac_h3_t5_suspended_child_does_not_drain_user_messages() {
  // AC-H3-T5: 親 Suspend 後に子 mailbox へ user message を積んでも、
  // suspend counter が非 0 である限り user queue は drain されない。
  // AC-H3 (再帰 Suspend 伝播) と MB-H1 (suspend-aware drain 制御) の
  // 結合契約を確認する。`process_all_system_messages` は Suspend を見た
  // 時点で counter を先に更新するため、同じ drain cycle で後続の user
  // message が処理されてしまわないこと自体が MB-H1 契約であり、本テストは
  // それを AC-H3 の再帰経路上でも崩さないことを示す。
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(240, 0), None, "parent".to_string(), &parent_props)
    .expect("create parent");
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let child_props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let child = ActorCell::create(state.clone(), Pid::new(241, 0), Some(parent.pid()), "child".to_string(), &child_props)
    .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());

  // 子の pre_start を先に完了させる（receive 観測可能な状態にしておく）。
  let mut child_invoker = ActorCellInvoker { cell: child.downgrade() };
  child_invoker.system_invoke(SystemMessage::Create).expect("child create");
  assert_eq!(log.lock().clone(), vec!["pre_start"]);

  // 親 Suspend → 子への Suspend 伝播 → 子 mailbox 側で明示 drain。
  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Suspend).expect("parent suspend");
  let _scheduled_system = child.new_dispatcher_shared().register_for_execution(&child.mailbox(), false, true);
  assert!(child.mailbox().is_suspended(), "AC-H3: 子は親 Suspend で suspended である");

  // suspended 状態の子へ user message を積む。
  assert!(child.actor_ref().try_tell(AnyMessage::new(())).is_ok(), "suspended でも enqueue 自体は成功する");
  assert_eq!(child.mailbox().user_len(), 1, "積んだ直後は queue に 1 件残る");

  // user-hint のみで drain を試みる。MB-H1 + AC-H3 の契約が守られていれば
  // user queue は処理されず、receive は呼ばれない。
  let _scheduled_user = child.new_dispatcher_shared().register_for_execution(&child.mailbox(), true, false);

  let snapshot = log.lock().clone();
  assert_eq!(snapshot, vec!["pre_start"], "AC-H3 × MB-H1: suspended 中の子は user message を drain してはならない");
  assert_eq!(
    child.mailbox().user_len(),
    1,
    "suspended 中は user queue が温存され、count が保持されていなければならない"
  );
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

#[test]
fn ac_h3_ext_t1_fresh_cell_has_no_failed_info() {
  // AC-H3 拡張: 新規 ActorCell の `_failed` は NoFailedInfo (Pekko の初期値)。
  // 失敗状態を持たないため is_failed / is_failed_fatally はいずれも false で、
  // perpetrator は None。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(250, 0), None, "fresh".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  assert!(!cell.is_failed(), "新規 cell は failed ではない");
  assert!(!cell.is_failed_fatally(), "新規 cell は failed_fatally ではない");
  assert_eq!(cell.perpetrator(), None, "新規 cell の perpetrator は None");
}

#[test]
fn ac_h3_ext_t2_set_failed_records_perpetrator() {
  // AC-H3 拡張: Pekko `setFailed(perpetrator)` 相当。fatally でない限り
  // FailedRef(perpetrator) を記録し、is_failed が true、perpetrator() が
  // 当該 Pid を返す。is_failed_fatally は false を維持する。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(251, 0), None, "with-perp".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  let perpetrator = Pid::new(252, 0);
  cell.set_failed(perpetrator);
  assert!(cell.is_failed(), "set_failed 後は is_failed が true");
  assert!(!cell.is_failed_fatally(), "set_failed (FailedRef) は fatally ではない");
  assert_eq!(cell.perpetrator(), Some(perpetrator), "perpetrator が正しく取得できる");
}

#[test]
fn ac_h3_ext_t3_clear_failed_resets_to_no_failed_info() {
  // AC-H3 拡張: Pekko `clearFailed()` は無条件で _failed = NoFailedInfo に戻す。
  // restart 完了後 (finishCreate / finishRecreate) に呼ばれることを想定し、
  // is_failed / is_failed_fatally がいずれも false に戻ることを確認する。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(253, 0), None, "clear".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  cell.set_failed(Pid::new(254, 0));
  assert!(cell.is_failed(), "事前条件: set_failed で failed 状態にする");

  cell.clear_failed();
  assert!(!cell.is_failed(), "clear_failed 後は is_failed が false");
  assert!(!cell.is_failed_fatally(), "clear_failed 後は is_failed_fatally も false");
  assert_eq!(cell.perpetrator(), None, "clear_failed 後 perpetrator は None");
}

#[test]
fn ac_h3_ext_t4_set_failed_fatally_marks_actor_dead() {
  // AC-H3 拡張: Pekko `setFailedFatally()` 相当。Kill や復旧不能な失敗で
  // 呼ばれ、_failed = FailedFatally を確定させる。is_failed と
  // is_failed_fatally の双方が true になり、perpetrator は None
  // (特定の子ではなく自身が fatal 失敗した) になる。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(255, 0), None, "fatal".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  cell.set_failed_fatally();
  assert!(cell.is_failed(), "fatally は failed のサブセット");
  assert!(cell.is_failed_fatally(), "set_failed_fatally 後は is_failed_fatally が true");
  assert_eq!(cell.perpetrator(), None, "fatally は perpetrator を持たない");
}

#[test]
fn ac_h3_ext_t5_set_failed_does_not_overwrite_fatally() {
  // AC-H3 拡張: Pekko `setFailed` は `_failed match { case FailedFatally => ... }`
  // ガードを持ち、fatally 状態の cell に対しては perpetrator 上書きを行わない。
  // Kill 直後の cell に再度 set_failed が呼ばれても fatally 状態が維持される。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(256, 0), None, "fatal-guarded".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  cell.set_failed_fatally();
  assert!(cell.is_failed_fatally(), "事前条件: fatally 状態にする");

  cell.set_failed(Pid::new(257, 0));
  assert!(cell.is_failed_fatally(), "AC-H3 拡張: fatally は set_failed で上書きされない");
  assert_eq!(cell.perpetrator(), None, "fatally の perpetrator は None のまま");
}

#[test]
fn ac_h3_ext_t6_clear_failed_resets_fatally() {
  // AC-H3 拡張: Pekko `clearFailed()` は無条件で _failed = NoFailedInfo にする
  // ため、fatally 状態もクリアされる。これは finishCreate / finishRecreate
  // 経路で fresh actor として再起動する際に必要な振る舞いである。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(258, 0), None, "clear-fatal".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  cell.set_failed_fatally();
  cell.clear_failed();
  assert!(!cell.is_failed(), "AC-H3 拡張: clear_failed は fatally もクリアする");
  assert!(!cell.is_failed_fatally());
}

// ============================================================================
// AC-M3: `report_failure` wires `set_failed(self.pid)` with `is_failed()` guard
// (Pekko `FaultHandling.scala:218-234` handleInvokeFailure parity).
// ============================================================================

#[test]
fn ac_m3_report_failure_records_self_as_perpetrator() {
  // Pekko `FaultHandling.scala:222`: case _ if !isFailed => setFailed(self)
  // 初回 `report_failure` 呼び出しで `FailedInfo::Child(self.pid)` が記録され、
  // `is_failed() == true` / `perpetrator() == Some(self.pid)` となる。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(260, 0), None, "ac-m3-self-perp".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  assert!(!cell.is_failed(), "事前条件: 新規 cell は failed ではない");

  cell.report_failure(&ActorError::recoverable("ac-m3-t1-boom"), None);

  assert!(cell.is_failed(), "AC-M3: report_failure 後は is_failed が true");
  assert_eq!(cell.perpetrator(), Some(cell.pid()), "AC-M3: perpetrator は self.pid");
  assert!(!cell.is_failed_fatally(), "AC-M3: 通常の report_failure は fatal ではない");
}

#[test]
fn ac_m3_duplicate_report_failure_preserves_perpetrator() {
  // Pekko `FaultHandling.scala:221`: `!isFailed` guard により、既に failed 中の
  // cell に対する重複 report_failure は perpetrator を overwrite しない。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(261, 0), None, "ac-m3-dup".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  cell.report_failure(&ActorError::recoverable("ac-m3-t2-first"), None);
  let perpetrator_after_first = cell.perpetrator();
  assert_eq!(perpetrator_after_first, Some(cell.pid()), "事前条件: 初回 report_failure で self.pid が記録");

  cell.report_failure(&ActorError::recoverable("ac-m3-t2-second"), None);

  assert_eq!(
    cell.perpetrator(),
    perpetrator_after_first,
    "AC-M3: 重複 report_failure は perpetrator を overwrite しない"
  );
  assert!(cell.is_failed(), "AC-M3: is_failed は維持される");
}

#[test]
fn ac_m3_report_failure_preserves_fatal_state() {
  // Pekko `FaultHandling.scala:79-82`: `setFailed` は FailedFatally を保持する。
  // fraktor-rs の `set_failed` 実装 (`actor_cell.rs:448`) も同じ guard を持ち、
  // さらに `report_failure` の `!is_failed()` guard で二重防御される。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(262, 0), None, "ac-m3-fatal".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  cell.set_failed_fatally();
  assert!(cell.is_failed_fatally(), "事前条件: set_failed_fatally で Fatal 状態");

  cell.report_failure(&ActorError::recoverable("ac-m3-t3-after-fatal"), None);

  assert!(cell.is_failed_fatally(), "AC-M3: Fatal 状態は downgrade されない");
  assert_eq!(cell.perpetrator(), None, "AC-M3: Fatal 状態では perpetrator は常に None");
}

#[test]
fn ac_m3_restart_clears_perpetrator() {
  // Pekko `FaultHandling.scala:284` finishRecreate: restart 完了時に clearFailed()。
  // fraktor-rs の既存配線 (`actor_cell.rs:1264`) で `finish_recreate` 内の
  // `recreate_actor` 直後に `clear_failed()` が走り、`FailedInfo::Child(_)`
  // が `FailedInfo::None` に戻ることを観測する。
  //
  // テスト戦略: orphan cell の `system.report_failure` は parent 無しの経路で
  // `SystemMessage::Stop` を自分自身に送る副作用があり、sync dispatcher 上で
  // inline 処理されて cell が terminated になる。この race を避けるため、
  // AC-H4-T1 と同じパターンで `set_failed` + `mailbox.suspend` を直接呼んで
  // failure の事前状態を再現する (本 change の `report_failure` wiring 自体は
  // `ac_m3_report_failure_records_self_as_perpetrator` などで別途 pin 済み)。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(263, 0), None, "ac-m3-restart".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  // AC-M3 `report_failure` 相当の事前状態を直接仕込む: perpetrator = self.pid、
  // mailbox suspended (fault_recreate の AC-H3 precondition)。
  cell.set_failed(cell.pid());
  cell.mailbox().suspend();
  assert_eq!(cell.perpetrator(), Some(cell.pid()), "事前条件: set_failed で perpetrator == self.pid");

  // supervisor directive Restart を simulation (SystemMessage::Recreate)
  let cause = ActorErrorReason::new("ac-m3-t4-restart-cause");
  invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate");

  assert!(!cell.is_failed(), "AC-M3: restart 完了後は is_failed が false");
  assert_eq!(cell.perpetrator(), None, "AC-M3: restart 完了後は perpetrator が None");

  // 次のサイクル: 新しい set_failed で新しい perpetrator が記録されることを確認
  cell.set_failed(cell.pid());
  assert_eq!(cell.perpetrator(), Some(cell.pid()), "AC-M3: restart 後の次のサイクルで perpetrator が再記録される");
}

#[test]
fn ac_m3_resume_clears_perpetrator() {
  // Pekko `FaultHandling.scala:150` faultResume: `finally clearFailed()`。
  // 本 change で `SystemMessage::Resume` arm に `clear_failed()` を追加したため、
  // supervisor directive Resume 経路でも state がクリアされる。
  //
  // テスト戦略: `ac_m3_restart_clears_perpetrator` と同じく直接 `set_failed` で
  // 事前状態を仕込み、orphan cell の Stop 副作用を回避する。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(264, 0), None, "ac-m3-resume".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  cell.set_failed(cell.pid());
  assert_eq!(cell.perpetrator(), Some(cell.pid()), "事前条件: set_failed で perpetrator == self.pid");

  // supervisor directive Resume を simulation
  invoker.system_invoke(SystemMessage::Resume).expect("resume");

  assert!(!cell.is_failed(), "AC-M3: Resume arm の clear_failed で is_failed が false");
  assert_eq!(cell.perpetrator(), None, "AC-M3: Resume arm の clear_failed で perpetrator が None");

  // 次のサイクル: 新しい set_failed で新しい perpetrator が記録されることを確認
  cell.set_failed(cell.pid());
  assert_eq!(cell.perpetrator(), Some(cell.pid()), "AC-M3: Resume 後の次のサイクルで perpetrator が再記録される");
}

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

#[test]
fn ac_h2_t1_register_child_keeps_container_normal() {
  // AC-H2: 子を登録しただけでは ChildrenContainer は Normal 状態を維持し、
  // is_terminating は false を返す。
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(270, 0), None, "normal-parent".to_string(), &parent_props)
    .expect("create parent");
  state.register_cell(parent.clone());

  let child_pid = Pid::new(271, 0);
  parent.register_child(child_pid);

  assert!(parent.children_state_is_normal(), "AC-H2: register_child 後も Normal 状態");
  assert!(!parent.children_state_is_terminating(), "AC-H2: Terminating ではない");
}

#[test]
#[ignore = "Phase A3 dependency: fault_terminate のみ post_stop 遅延 + Terminating(Termination) 遷移を要する。pekko-restart-completion の scope 外"]
fn ac_h2_t2_fault_terminate_with_children_transitions_to_terminating() {
  // AC-H2 / Phase A3: handle_stop (fault_terminate) は live child がある間
  // post_stop と mark_terminated を遅延し、ChildrenContainer を Terminating(Termination)
  // に遷移させる。現状 handle_stop は post_stop を同期実行するため本テストは
  // Phase A3 の fault_terminate 配線まで ignore。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let parent_props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let parent = ActorCell::create(state.clone(), Pid::new(272, 0), None, "term-parent".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child =
    ActorCell::create(state.clone(), Pid::new(273, 0), Some(parent.pid()), "term-child".to_string(), &child_props)
      .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");
  assert_eq!(log.lock().clone(), vec!["pre_start"]);

  parent_invoker.system_invoke(SystemMessage::Stop).expect("parent stop with live child");

  let snapshot = log.lock().clone();
  assert_eq!(
    snapshot,
    vec!["pre_start"],
    "AC-H2: live child がある fault_terminate は post_stop を遅延しなければならない"
  );
  assert!(
    parent.children_state_is_terminating(),
    "AC-H2: fault_terminate 後は ChildrenContainer が Terminating(Termination) に遷移"
  );
  assert!(!parent.children_state_is_normal(), "AC-H2: Terminating 中は is_normal=false");
}

#[test]
#[ignore = "Phase A3 dependency: finish_terminate dispatch on Termination state-change is out of scope for pekko-restart-completion"]
fn ac_h2_t3_finish_terminate_runs_post_stop_after_last_child() {
  // AC-H2 / Phase A3: Terminating(Termination) 状態で最後の子が
  // handle_death_watch_notification されたとき finish_terminate が起動し、
  // post_stop が実行される。本 change (pekko-restart-completion) は
  // `Recreation` state-change のみ dispatch し、`Termination` は
  // `// TODO(Phase A3)` マーク（tasks.md 4.2 step 8）のため本テストは ignore。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let parent_props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let parent = ActorCell::create(state.clone(), Pid::new(274, 0), None, "term-parent2".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child =
    ActorCell::create(state.clone(), Pid::new(275, 0), Some(parent.pid()), "term-child2".to_string(), &child_props)
      .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());
  parent.register_watching(child.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");
  parent_invoker.system_invoke(SystemMessage::Stop).expect("parent stop with live child");
  assert_eq!(log.lock().clone(), vec!["pre_start"], "事前条件: post_stop は遅延されている");

  // 最後の子が terminated → finish_terminate 経路で post_stop が完了する。
  parent.handle_death_watch_notification(child.pid()).expect("handle_death_watch_notification");

  let snapshot = log.lock().clone();
  assert_eq!(
    snapshot,
    vec!["pre_start", "post_stop"],
    "AC-H2: 最後の子 termination → finish_terminate 経由で post_stop が呼ばれる"
  );
  assert!(parent.children().is_empty(), "AC-H2: 子 termination 後に children() は空");
}

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

#[test]
fn ac_h4_t1_fault_recreate_no_children_runs_full_restart_cycle() {
  // AC-H4: live child がいない fault_recreate(cause) は immediate finishRecreate
  // を実行し、pre_restart(cause) → recreate_actor → post_restart(cause) の順序
  // でライフサイクル callback が走る。RestartLifecycleRecorderActor は
  // post_restart 既定 (= pre_start 委譲) を使わずに override で記録するため、
  // log の最終要素は post_restart:cause となる (kernel は restart 経路で
  // 自動的に pre_start を呼ばないことも併せて確認する)。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || RestartLifecycleRecorderActor::new(log.clone())
  });
  let cell = ActorCell::create(state.clone(), Pid::new(300, 0), None, "no-children".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  assert_eq!(log.lock().clone(), vec!["pre_start".to_string()]);

  // fault_recreate の前提として mailbox を suspend (Pekko は failure 経路で
  // 既に suspend されている前提で faultRecreate を呼ぶ)。
  cell.mailbox().suspend();
  let cause = ActorErrorReason::new("ac-h4-t1-cause");
  invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate");

  let snapshot = log.lock().clone();
  assert_eq!(
    snapshot,
    vec!["pre_start".to_string(), "pre_restart:ac-h4-t1-cause".to_string(), "post_restart:ac-h4-t1-cause".to_string(),],
    "AC-H4: 子なし fault_recreate は pre_restart → post_restart を即座に完走させる"
  );
  assert!(!cell.mailbox().is_suspended(), "AC-H4: finishRecreate 完了時に mailbox は resume されていなければならない");
}

#[test]
fn ac_h4_t2_fault_recreate_with_children_defers_finish_recreate() {
  // AC-H4: live child がある fault_recreate(cause) は pre_restart(cause) を
  // 1 回だけ呼んだ後、ChildrenContainer を Recreation(cause) suspend reason で
  // 待機状態に遷移させ、post_restart は遅延される。
  // Pekko `FaultHandling.scala:215-237`: childrenRefs が Normal でないとき
  // `setChildrenTerminationReason(Recreation(cause))` で待機を仕込む。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let parent_props = Props::from_fn({
    let log = log.clone();
    move || RestartLifecycleRecorderActor::new(log.clone())
  });
  let parent = ActorCell::create(state.clone(), Pid::new(310, 0), None, "wait-parent".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child =
    ActorCell::create(state.clone(), Pid::new(311, 0), Some(parent.pid()), "wait-child".to_string(), &child_props)
      .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());
  // Pekko parity: override `pre_restart` では stop_all_children が走らないので
  // deferred 経路の前提 (children_state が Terminating) を明示的に仕込む必要が
  // ある。default flow の `context.stop(child)` → `shallDie(child)` を手動で
  // 再現する。
  parent.mark_child_dying(child.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");
  assert_eq!(log.lock().clone(), vec!["pre_start".to_string()]);

  parent.mailbox().suspend();
  let cause = ActorErrorReason::new("ac-h4-t2-cause");
  parent_invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate must not surface error");

  let snapshot = log.lock().clone();
  assert_eq!(
    snapshot,
    vec!["pre_start".to_string(), "pre_restart:ac-h4-t2-cause".to_string(),],
    "AC-H4: live child がある状態の fault_recreate は post_restart を遅延しなければならない"
  );
  assert!(
    parent.children_state_is_terminating(),
    "AC-H4: fault_recreate 後は ChildrenContainer が Terminating(Recreation(cause)) に遷移"
  );
  assert!(parent.mailbox().is_suspended(), "AC-H4: finishRecreate が遅延されている間 mailbox は suspended のまま");
}

#[test]
fn ac_h4_t3_finish_recreate_fires_after_last_child_terminated() {
  // AC-H4: Terminating(Recreation(cause)) 状態の親に対し、最後の子が
  // handle_terminated されると `removeChildAndGetStateChange` が
  // SuspendReason::Recreation(cause) を返し、これを契機に finishRecreate(cause)
  // が起動して post_restart(cause) が完了する。Pekko
  // `FaultHandling.scala:handleChildTerminated` の状態遷移ハンドリングを
  // kernel 層で観測する。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let parent_props = Props::from_fn({
    let log = log.clone();
    move || RestartLifecycleRecorderActor::new(log.clone())
  });
  let parent = ActorCell::create(state.clone(), Pid::new(320, 0), None, "finish-parent".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child =
    ActorCell::create(state.clone(), Pid::new(321, 0), Some(parent.pid()), "finish-child".to_string(), &child_props)
      .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());
  // AC-H5 pre-wiring: spawn_with_parent 自動配線と同等の supervision watch を
  // 手動登録する（本テストは spawn_with_parent を通さず register_child のみ呼ぶため）。
  parent.register_watching(child.pid());
  // Pekko parity: override `pre_restart` は stop_all_children を呼ばないため、
  // deferred 経路に乗るには事前に children_state を Terminating へ遷移させる
  // 必要がある（`context.stop(child)` → `shallDie(child)` と等価）。
  parent.mark_child_dying(child.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");
  parent.mailbox().suspend();
  let cause = ActorErrorReason::new("ac-h4-t3-cause");
  parent_invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate");
  assert_eq!(
    log.lock().clone(),
    vec!["pre_start".to_string(), "pre_restart:ac-h4-t3-cause".to_string(),],
    "事前条件: 子待機中で post_restart は未実行"
  );

  // 最後の子が terminated → finishRecreate 起動 → post_restart(cause) 完了
  parent.handle_death_watch_notification(child.pid()).expect("handle_death_watch_notification");

  let snapshot = log.lock().clone();
  assert_eq!(
    snapshot,
    vec!["pre_start".to_string(), "pre_restart:ac-h4-t3-cause".to_string(), "post_restart:ac-h4-t3-cause".to_string(),],
    "AC-H4: 最後の子 termination 後に finishRecreate(cause) が起動し post_restart が完了する"
  );
  assert!(parent.children_state_is_normal(), "AC-H4: finishRecreate 後 ChildrenContainer は Normal/Empty に戻る");
  assert!(!parent.mailbox().is_suspended(), "AC-H4: finishRecreate 完了時に mailbox は resume される");
}

#[test]
fn ac_h4_t4_recreate_is_no_op_when_failed_fatally() {
  // AC-H4: Pekko `FaultHandling.scala:215-220` faultRecreate は
  // `isFailedFatally` が true の間は no-op (再起動を試みず、actor を null の
  // ままにする)。fraktor-rs では `set_failed_fatally` で fatal 状態を確定
  // させた後に SystemMessage::Recreate(cause) を投げても、追加のライフサイクル
  // callback が呼ばれないことを観測する。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || RestartLifecycleRecorderActor::new(log.clone())
  });
  let cell = ActorCell::create(state.clone(), Pid::new(330, 0), None, "fatal-noop".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  assert_eq!(log.lock().clone(), vec!["pre_start".to_string()]);

  cell.set_failed_fatally();
  assert!(cell.is_failed_fatally(), "事前条件: fatally 状態を確定させる");

  cell.mailbox().suspend();
  let cause = ActorErrorReason::new("ac-h4-fatal-cause");
  invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate must not surface error");

  let snapshot = log.lock().clone();
  assert_eq!(
    snapshot,
    vec!["pre_start".to_string()],
    "AC-H4: is_failed_fatally が true の間 fault_recreate は no-op で、追加 callback を呼ばない"
  );
  assert!(cell.is_failed_fatally(), "AC-H4: fatally 状態は fault_recreate (no-op) を経ても維持される");
}

#[test]
fn ac_h4_t5_recreate_preserves_cause_payload_distinctly() {
  // AC-H4: SystemMessage::Recreate(ActorErrorReason) は cause payload を
  // round-trip で保持する。異なる cause 文字列を 2 回続けて投げたとき、
  // pre_restart / post_restart に渡る reason がそれぞれの payload と一致
  // することを観測する。これは AC-H4 の「failureCause を pre_restart と
  // post_restart の両方に同じ参照で渡す」契約を行頭で確認する。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || RestartLifecycleRecorderActor::new(log.clone())
  });
  let cell = ActorCell::create(state.clone(), Pid::new(331, 0), None, "cause-distinct".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  cell.mailbox().suspend();
  invoker.system_invoke(SystemMessage::Recreate(ActorErrorReason::new("first"))).expect("first recreate");
  cell.mailbox().suspend();
  invoker.system_invoke(SystemMessage::Recreate(ActorErrorReason::new("second"))).expect("second recreate");

  let snapshot = log.lock().clone();
  assert_eq!(
    snapshot,
    vec![
      "pre_start".to_string(),
      "pre_restart:first".to_string(),
      "post_restart:first".to_string(),
      "pre_restart:second".to_string(),
      "post_restart:second".to_string(),
    ],
    "AC-H4: 各 Recreate(cause) の payload は対応する pre_restart / post_restart に欠損なく届く"
  );
}

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

#[test]
fn al_h1_t1_default_pre_restart_calls_post_stop_and_default_post_restart_calls_pre_start() {
  // AL-H1: 子なしで `LifecycleRecorderActor` (pre_restart / post_restart 既定実装)
  // を Recreate すると、既定 pre_restart が post_stop を呼び、続いて既定
  // post_restart が pre_start を呼ぶ。kernel は restart 経路で pre_start を直接
  // 呼ばないため、最終ログは pre_start (Create) → post_stop (default pre_restart)
  // → pre_start (default post_restart 経由) の順で観測される。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell = ActorCell::create(state.clone(), Pid::new(800, 0), None, "al-h1-t1".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  assert_eq!(log.lock().clone(), vec!["pre_start"]);

  cell.mailbox().suspend();
  let cause = ActorErrorReason::new("al-h1-t1-cause");
  invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate");

  let snapshot = log.lock().clone();
  assert_eq!(
    snapshot,
    vec!["pre_start", "post_stop", "pre_start"],
    "AL-H1: 既定 pre_restart → post_stop と 既定 post_restart → pre_start の連鎖が走る"
  );
  assert!(!cell.mailbox().is_suspended(), "AL-H1: finishRecreate 完了時に mailbox は resume されていなければならない");
}

#[test]
fn al_h1_t2_default_pre_restart_stops_children_and_defers_finish_recreate() {
  // AL-H1: 既定 pre_restart は stop_all_children を呼ぶことで子を terminate
  // キューに乗せた後、自身の post_stop を呼ぶ。childrenRefs は live child を
  // 残したまま Terminating(Recreation) へ遷移するため finishRecreate は遅延され、
  // 子が handle_terminated されるタイミングで post_restart (= 既定の pre_start
  // 委譲) が走る。
  //
  // Sync-dispatch parity は `ActorCell::fault_recreate` が `pre_restart` を
  // `MessageDispatcherShared::run_with_drive_guard` でラップすることで成立する。
  // guard が `ExecutorShared::running` を事前に claim するため、
  // `stop_all_children` が child へ発行する `SystemMessage::Stop` は既存
  // trampoline の pending に積まれるだけで parent の呼び出しスタック上では
  // drain されない。後続の `parent.handle_death_watch_notification(child)` が
  // `remove_child_and_get_state_change` で `Recreation(cause)` を観測し
  // `finish_recreate` を起動する。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let parent_props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let parent = ActorCell::create(state.clone(), Pid::new(810, 0), None, "al-h1-t2".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child =
    ActorCell::create(state.clone(), Pid::new(811, 0), Some(parent.pid()), "al-h1-t2-child".to_string(), &child_props)
      .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());
  // AC-H5 pre-wiring: spawn_with_parent 自動配線と同等の supervision watch を
  // 手動登録する。`register_supervision_watching` で `WatchKind::Supervision` を
  // 使うことで、既定 `pre_restart` の `stop_all_children` が呼ぶ
  // `unregister_watching`（User kind のみ除去）の影響を受けず、後続の
  // `handle_death_watch_notification` が `watching_contains_pid` で通過する。
  parent.register_supervision_watching(child.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");
  assert_eq!(log.lock().clone(), vec!["pre_start"]);

  parent.mailbox().suspend();
  let cause = ActorErrorReason::new("al-h1-t2-cause");
  parent_invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate");

  let mid_snapshot = log.lock().clone();
  assert_eq!(
    mid_snapshot,
    vec!["pre_start", "post_stop"],
    "AL-H1: 既定 pre_restart は post_stop を呼ぶが post_restart は live child を待つため遅延"
  );
  assert!(
    parent.children_state_is_terminating(),
    "AL-H1: 子 stop 待ちの間は ChildrenContainer が Terminating(Recreation)"
  );
  assert!(!parent.children_state_is_normal(), "AL-H1: Terminating 中は is_normal=false");

  // 最後の子が terminated → finishRecreate → recreate_actor → 既定 post_restart
  // → 既定 pre_start 委譲、の順序で続きが走る。
  parent.handle_death_watch_notification(child.pid()).expect("handle_death_watch_notification");

  let final_snapshot = log.lock().clone();
  assert_eq!(
    final_snapshot,
    vec!["pre_start", "post_stop", "pre_start"],
    "AL-H1: 子 termination 後に finishRecreate 経由で 既定 post_restart が pre_start を呼ぶ"
  );
  assert!(parent.children().is_empty(), "AL-H1: finishRecreate 後は children() は空");
  assert!(!parent.mailbox().is_suspended(), "AL-H1: finishRecreate 完了後は mailbox を resume");
  assert!(parent.children_state_is_normal(), "AL-H1: finishRecreate 後は ChildrenContainer が Normal に戻る");
}

#[test]
fn al_h1_t2_default_pre_restart_with_multiple_children_defers_finish_recreate_until_last() {
  // AL-H1: child が 2 件以上ある場合、最後の child の
  // `handle_death_watch_notification` が届くまで `finish_recreate` が起動しない
  // ことを確認する。中間の child DWN では `remove_child_and_get_state_change` が
  // `None` を返し（container が Terminating に留まり to_die が非空のまま）、
  // 最後の child DWN で初めて `Some(Recreation(cause))` → `finish_recreate` が起動する。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let parent_props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let parent = ActorCell::create(state.clone(), Pid::new(813, 0), None, "al-h1-t2-multi".to_string(), &parent_props)
    .expect("create parent");
  let child_a_props = Props::from_fn(|| ProbeActor);
  let child_a = ActorCell::create(
    state.clone(),
    Pid::new(814, 0),
    Some(parent.pid()),
    "al-h1-t2-multi-child-a".to_string(),
    &child_a_props,
  )
  .expect("create child_a");
  let child_b_props = Props::from_fn(|| ProbeActor);
  let child_b = ActorCell::create(
    state.clone(),
    Pid::new(815, 0),
    Some(parent.pid()),
    "al-h1-t2-multi-child-b".to_string(),
    &child_b_props,
  )
  .expect("create child_b");
  state.register_cell(parent.clone());
  state.register_cell(child_a.clone());
  state.register_cell(child_b.clone());
  parent.register_child(child_a.pid());
  parent.register_child(child_b.pid());
  // 両 child を supervision watch に登録（User kind だと stop_all_children で除去される）
  parent.register_supervision_watching(child_a.pid());
  parent.register_supervision_watching(child_b.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");
  assert_eq!(log.lock().clone(), vec!["pre_start"]);

  parent.mailbox().suspend();
  let cause = ActorErrorReason::new("al-h1-t2-multi-cause");
  parent_invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate");

  // Recreate 直後: children が 2 件残り、Terminating(Recreation) で待機中。
  assert_eq!(parent.children().len(), 2, "両 child が children_state の to_die に残存");
  assert!(parent.children_state_is_terminating(), "子 stop 待ちで Terminating(Recreation)");
  let mid_snapshot = log.lock().clone();
  assert_eq!(mid_snapshot, vec!["pre_start", "post_stop"], "post_restart は最後の child 終了まで遅延");

  // child_a の DWN → まだ child_b が to_die に残るので finish_recreate は起動しない
  parent.handle_death_watch_notification(child_a.pid()).expect("handle_death_watch_notification A");
  assert!(parent.children_state_is_terminating(), "child_a 除去後も to_die に child_b が残存するので Terminating 継続");
  assert_eq!(parent.children().len(), 1, "child_a のみ children_state から除去される");
  let after_a_snapshot = log.lock().clone();
  assert_eq!(
    after_a_snapshot,
    vec!["pre_start", "post_stop"],
    "child_a の DWN 処理中に finish_recreate は起動しない（中間 state_change=None）"
  );

  // child_b の DWN → 最後の child なので finish_recreate が起動し post_restart → pre_start
  parent.handle_death_watch_notification(child_b.pid()).expect("handle_death_watch_notification B");
  let final_snapshot = log.lock().clone();
  assert_eq!(
    final_snapshot,
    vec!["pre_start", "post_stop", "pre_start"],
    "最後の child_b DWN で finish_recreate → 既定 post_restart → pre_start が走る"
  );
  assert!(parent.children_state_is_normal(), "finish_recreate 後に Normal/Empty へ戻る");
  assert!(parent.children().is_empty(), "finish_recreate 後 children は空");
  assert!(!parent.mailbox().is_suspended(), "finish_recreate 後 mailbox を resume");
}

#[test]
fn al_h1_t3_overridden_pre_restart_replaces_default_child_stop() {
  // AL-H1: pre_restart を override した actor (RestartLifecycleRecorderActor) は
  // 既定の stop_all_children + post_stop を実行しない。kernel は override の戻り値
  // 後に stop_all_children を再委譲しないため、children は override の責任のままで
  // あり、post_stop も呼ばれない。post_restart も同様に override で完結し pre_start
  // は呼ばれない。AC-H4 T2/T3 は遅延 finishRecreate の経路を扱うが、本ケースは
  // 「override が default を完全に置き換える」契約に焦点を当てる。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let parent_props = Props::from_fn({
    let log = log.clone();
    move || RestartLifecycleRecorderActor::new(log.clone())
  });
  let parent = ActorCell::create(state.clone(), Pid::new(820, 0), None, "al-h1-t3".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child =
    ActorCell::create(state.clone(), Pid::new(821, 0), Some(parent.pid()), "al-h1-t3-child".to_string(), &child_props)
      .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());
  // Pekko parity: override `pre_restart` は stop_all_children を呼ばないため、
  // deferred 経路を観測するには事前に `shall_die` 経由で children_state を
  // Terminating へ遷移させる必要がある。
  parent.mark_child_dying(child.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");
  assert_eq!(log.lock().clone(), vec!["pre_start".to_string()]);

  parent.mailbox().suspend();
  let cause = ActorErrorReason::new("al-h1-t3-cause");
  parent_invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate");

  // override pre_restart は children を stop しないが、`mark_child_dying` で
  // 事前に Terminating(UserRequest) に遷移させているため fault_recreate は
  // `set_children_termination_reason(Recreation)` で reason を上書きして deferred に入る。
  let snapshot = log.lock().clone();
  assert_eq!(
    snapshot,
    vec!["pre_start".to_string(), "pre_restart:al-h1-t3-cause".to_string()],
    "AL-H1: override pre_restart は default の post_stop を委譲しない"
  );
  assert!(
    parent.children().contains(&child.pid()),
    "AL-H1: override pre_restart は default の stop_all_children を委譲しないため child は残る"
  );
  assert!(
    parent.children_state_is_terminating(),
    "AL-H1: live child があるため ChildrenContainer は Terminating(Recreation) で待機"
  );
}

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

#[test]
fn ac_h5_t1_terminated_queued_starts_empty_on_fresh_cell() {
  // AC-H5: 新規 ActorCell の `terminated_queued()` は空配列を返す。
  // これは「初期状態では何も deliver されていない」契約のベースライン。
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell = ActorCell::create(state.clone(), Pid::new(500, 0), None, "h5-empty".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());

  assert!(cell.terminated_queued().is_empty(), "AC-H5: 新規 cell は terminated_queued を持たない");
  assert!(!cell.is_watching(Pid::new(501, 0)), "AC-H5: 新規 cell は何も watch していない");
}

#[test]
fn ac_h5_t2_death_watch_notification_removes_watching_entry_and_calls_on_terminated() {
  // AC-H5: `handle_death_watch_notification(pid)` は watching から pid を除去し、
  // on_terminated を kernel 直接呼びで起動する。`terminated_queued` は push→dispatch→pop
  // で短命な dedup marker として使うため、呼び出し後には残らない（spec design 参照）。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let watcher_props = Props::from_fn({
    let log = log.clone();
    move || RecordingActor::new(log.clone())
  });
  let watcher = ActorCell::create(state.clone(), Pid::new(510, 0), None, "h5-watcher".to_string(), &watcher_props)
    .expect("create watcher");
  state.register_cell(watcher.clone());

  let target_pid = Pid::new(511, 0);
  watcher.register_watching(target_pid);
  assert!(watcher.is_watching(target_pid), "AC-H5: register_watching で watching set に入る");

  let mut invoker = ActorCellInvoker { cell: watcher.downgrade() };
  invoker.system_invoke(SystemMessage::DeathWatchNotification(target_pid)).expect("death-watch-notification");

  assert!(!watcher.is_watching(target_pid), "AC-H5: handle 完了後 watching から target が除去される");
  assert!(
    watcher.terminated_queued().is_empty(),
    "AC-H5: terminated_queued は handle 完了後クリアされる (dedup 保持期間は handle 内のみ)"
  );
  assert_eq!(log.lock().clone(), vec![target_pid], "AC-H5: on_terminated が kernel 直接呼びで起動される");
}

#[test]
fn ac_h5_t3_duplicate_death_watch_notifications_dedupe_via_watching_removal() {
  // AC-H5 dedup: 同じ pid に対する DeathWatchNotification を 2 回送っても、
  // 1 回目の handle で watching から pid を除去するため、2 回目は
  // `watching_contains_pid` 判定で弾かれ on_terminated は 1 回しか呼ばれない。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let watcher_props = Props::from_fn({
    let log = log.clone();
    move || RecordingActor::new(log.clone())
  });
  let watcher = ActorCell::create(state.clone(), Pid::new(520, 0), None, "h5-dedup".to_string(), &watcher_props)
    .expect("create watcher");
  state.register_cell(watcher.clone());

  let target_pid = Pid::new(521, 0);
  watcher.register_watching(target_pid);

  let mut invoker = ActorCellInvoker { cell: watcher.downgrade() };
  invoker.system_invoke(SystemMessage::DeathWatchNotification(target_pid)).expect("dwn-1");
  invoker.system_invoke(SystemMessage::DeathWatchNotification(target_pid)).expect("dwn-2");

  assert_eq!(
    log.lock().clone(),
    vec![target_pid],
    "AC-H5: 重複した DeathWatchNotification でも on_terminated は 1 回のみ起動される"
  );
  assert!(
    watcher.terminated_queued().is_empty(),
    "AC-H5: 2 回目は watching_contains_pid false で弾かれ terminated_queued に残らない"
  );
}

#[test]
fn ac_h5_t4_death_watch_notification_for_unwatched_target_is_dropped() {
  // AC-H5: Pekko `watchedActorTerminated` 入口の `if (watchingContains(actor))` 分岐:
  //   watching に存在しない pid の DeathWatchNotification は完全に破棄される。
  //   user queue へも何も投入されず、terminated_queued にも入らない。
  let state = ActorSystem::new_empty().state();
  let watcher_props = Props::from_fn(|| ProbeActor);
  let watcher = ActorCell::create(state.clone(), Pid::new(530, 0), None, "h5-unwatched".to_string(), &watcher_props)
    .expect("create watcher");
  state.register_cell(watcher.clone());

  // watching に登録せずに DeathWatchNotification を送る。
  let stranger_pid = Pid::new(531, 0);
  let mut invoker = ActorCellInvoker { cell: watcher.downgrade() };
  invoker.system_invoke(SystemMessage::DeathWatchNotification(stranger_pid)).expect("dwn-stranger");

  assert!(
    watcher.terminated_queued().is_empty(),
    "AC-H5: watch していない pid からの DeathWatchNotification は terminated_queued に入らない"
  );
  assert!(!watcher.is_watching(stranger_pid), "AC-H5: dwn 受信が watching set を変えてはならない");
}

#[test]
fn ac_h5_t5_unwatch_removes_watching_and_terminated_queued_entries() {
  // AC-H5: `unregister_watching` は watching と terminated_queued の両方から
  // target を取り除く。DWN 処理の before/during に該当エントリを残さない契約。
  let state = ActorSystem::new_empty().state();
  let watcher_props = Props::from_fn(|| ProbeActor);
  let watcher = ActorCell::create(state.clone(), Pid::new(540, 0), None, "h5-unwatch".to_string(), &watcher_props)
    .expect("create watcher");
  state.register_cell(watcher.clone());

  let target_pid = Pid::new(541, 0);
  watcher.register_watching(target_pid);
  assert!(watcher.is_watching(target_pid), "事前条件: watching に target が居る");

  watcher.unregister_watching(target_pid);

  assert!(!watcher.is_watching(target_pid), "AC-H5: unregister_watching で watching set から外れる");
  assert!(
    watcher.terminated_queued().is_empty(),
    "AC-H5: unregister_watching は terminated_queued もクリアする (race 対策)"
  );
}

#[test]
fn ac_h5_t6_handle_death_watch_notification_cleans_terminated_queued_and_watching() {
  // AC-H5: spec design 通り、`handle_death_watch_notification` は push → dispatch →
  // pop を atomic に行う。戻り時に terminated_queued は空で、watching からも
  // target が除去済み。同一 pid の後続 DWN は silently drop される。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let watcher_props = Props::from_fn({
    let log = log.clone();
    move || RecordingActor::new(log.clone())
  });
  let watcher = ActorCell::create(state.clone(), Pid::new(550, 0), None, "h5-clear".to_string(), &watcher_props)
    .expect("create watcher");
  state.register_cell(watcher.clone());

  let target_pid = Pid::new(551, 0);
  watcher.register_watching(target_pid);

  let mut invoker = ActorCellInvoker { cell: watcher.downgrade() };
  invoker.system_invoke(SystemMessage::DeathWatchNotification(target_pid)).expect("dwn");

  assert!(watcher.terminated_queued().is_empty(), "AC-H5: handle 完了後 terminated_queued は空");
  assert!(!watcher.is_watching(target_pid), "AC-H5: handle 完了後 watching から除去");
  assert_eq!(log.lock().clone(), vec![target_pid], "AC-H5: on_terminated は 1 回呼ばれる");
}

#[test]
fn ac_h5_user_unwatch_preserves_supervision_watch() {
  // AC-H5 (WatchKind 分離): parent が child を user-level `watch` → `unwatch` しても
  // `Supervision` 登録は保持されるため、child 停止後の `DeathWatchNotification` は
  // `watching_contains_pid` 判定を通り抜けて handle される。
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(560, 0), None, "h5-kind".to_string(), &parent_props)
    .expect("create parent");
  state.register_cell(parent.clone());

  let child_pid = Pid::new(561, 0);
  // spawn_with_parent 相当の supervision 登録を模擬する。
  parent.register_supervision_watching(child_pid);
  parent.register_watching(child_pid); // user-level watch

  // user-level unwatch は User エントリだけ削除し、Supervision は残す。
  parent.unregister_watching(child_pid);

  assert!(
    parent.is_watching(child_pid),
    "AC-H5: User 登録を外しても Supervision が残るため watching_contains_pid は true"
  );

  // この状態で DeathWatchNotification が届けば handler は走る。
  let mut invoker = ActorCellInvoker { cell: parent.downgrade() };
  invoker
    .system_invoke(SystemMessage::DeathWatchNotification(child_pid))
    .expect("dwn should proceed since supervision watch survives unwatch");

  assert!(!parent.is_watching(child_pid), "AC-H5: handle 完了後は User / Supervision 両方とも除去される");
}

// === AC-M4a: watch_registration_kind query ============================
//
// Pekko `DeathWatch.scala:104` `watching.get(actor)` の 3 値セマンティクスを
// fraktor-rs の split data structure (`watching` + `watch_with_messages`) と
// `WatchKind::User` フィルタで合成できることを検証する。

#[test]
fn watch_registration_kind_returns_none_for_unknown_target() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(500, 0), None, "cell".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  assert_eq!(cell.watch_registration_kind(Pid::new(501, 0)), WatchRegistrationKind::None);
}

#[test]
fn watch_registration_kind_returns_plain_for_user_watch_only() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(510, 0), None, "cell".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());
  let target = Pid::new(511, 0);

  cell.register_watching(target);

  assert_eq!(cell.watch_registration_kind(target), WatchRegistrationKind::Plain);
}

#[test]
fn watch_registration_kind_returns_with_message_when_watch_with_registered() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(520, 0), None, "cell".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());
  let target = Pid::new(521, 0);

  cell.register_watching(target);
  cell.register_watch_with(target, AnyMessage::new(42_i32));

  assert_eq!(cell.watch_registration_kind(target), WatchRegistrationKind::WithMessage);
}

#[test]
fn watch_registration_kind_ignores_supervision_only_entry() {
  // 親 cell が子を spawn しただけで spawn_child_watched していない状態を模す。
  // Supervision watch のみが register される場合、user-level duplicate check は
  // 対象外として None を返す必要がある (Decision 2)。
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(530, 0), None, "parent".to_string(), &parent_props)
    .expect("create parent cell");
  state.register_cell(parent.clone());

  let child_pid = Pid::new(531, 0);
  parent.register_child(child_pid);

  assert_eq!(
    parent.watch_registration_kind(child_pid),
    WatchRegistrationKind::None,
    "Supervision kind の watching entry は user-level 判定に影響しない"
  );
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
