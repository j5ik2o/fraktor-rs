use alloc::{
  format,
  string::{String, ToString},
  vec,
  vec::Vec,
};
use core::{hint::spin_loop, num::NonZeroUsize, time::Duration};

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use super::{ActorCell, ActorCellInvoker};
use crate::core::kernel::{
  actor::{
    Actor, ActorContext, Pid, ReceiveTimeoutState,
    children_container::ChildrenContainer,
    error::{ActorError, ActorErrorReason},
    messaging::{
      ActorIdentity, AnyMessage, AnyMessageView, Identify, Kill, PoisonPill, message_invoker::MessageInvoker,
      system_message::SystemMessage,
    },
    props::{MailboxConfig, Props},
    supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyConfig, SupervisorStrategyKind},
    suspend_reason::SuspendReason,
  },
  dispatch::mailbox::{MailboxOverflowStrategy, MailboxPolicy},
  system::ActorSystem,
};

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
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 1, Duration::from_secs(1), |_| {
      SupervisorDirective::Resume
    })
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
    second_result.is_ok() || matches!(second_result, Err(crate::core::kernel::actor::error::SendError::Closed(_))),
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

#[test]
fn register_watch_with_replaces_previous_entry_for_same_target() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(74, 0), None, "watcher".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let target_pid = Pid::new(75, 0);
  cell.register_watch_with(target_pid, AnyMessage::new(1_i32));
  cell.register_watch_with(target_pid, AnyMessage::new(2_i32));

  // 後から登録した値（2）で上書きされていることを検証
  let msg = cell.take_watch_with_message(target_pid).expect("watch_with メッセージが存在すること");
  assert_eq!(*msg.payload().downcast_ref::<i32>().expect("i32 にダウンキャスト"), 2);
  assert!(cell.take_watch_with_message(target_pid).is_none());
}

#[test]
fn handle_terminated_skips_on_terminated_when_watch_with_registered() {
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
  let result = watcher.handle_terminated(target_pid);
  assert!(result.is_ok());
  assert!(log.lock().is_empty(), "on_terminated should not be called when watch_with is registered");
}

#[test]
fn handle_terminated_removes_child_from_children() {
  // AC-H2 統合テスト: Pekko `Children.scala:327` (`handleChildTerminated`) に相当。
  // `register_child` で追加された子が `handle_terminated` によって children() から
  // 取り除かれることを、kernel 層の ChildrenContainer state machine 経由で検証する。
  // これは `remove_child_and_get_state_change` が `handle_terminated` 内で
  // 呼び出されることで実現される（戻り値 Option<SuspendReason> は今回未使用、
  // AC-H4 で `finish_recreate` 発火に用いる）。
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(100, 0), None, "parent".to_string(), &parent_props)
    .expect("create parent");
  state.register_cell(parent.clone());

  let child_pid = Pid::new(101, 0);
  parent.register_child(child_pid);
  assert_eq!(parent.children(), vec![child_pid]);

  parent.handle_terminated(child_pid).expect("handle_terminated should succeed");

  assert!(
    parent.children().is_empty(),
    "children() は handle_terminated 後に空になる必要がある"
  );
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
  let child =
    ActorCell::create(state.clone(), Pid::new(201, 0), Some(parent.pid()), "child".to_string(), &child_props)
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

  assert!(
    child.mailbox().is_suspended(),
    "AC-H3: 親 Suspend 後、子 mailbox は suspended に遷移していなければならない"
  );
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
  let child =
    ActorCell::create(state.clone(), Pid::new(211, 0), Some(parent.pid()), "child".to_string(), &child_props)
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
  let child =
    ActorCell::create(state.clone(), Pid::new(221, 0), Some(parent.pid()), "child".to_string(), &child_props)
      .expect("create child");
  let grandchild_props = Props::from_fn(|| ProbeActor);
  let grandchild = ActorCell::create(
    state.clone(),
    Pid::new(222, 0),
    Some(child.pid()),
    "grandchild".to_string(),
    &grandchild_props,
  )
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

  assert!(
    child.mailbox().is_suspended(),
    "AC-H3: 第 1 段 (子) は親 Suspend 後に suspended になっていなければならない"
  );
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
  let child =
    ActorCell::create(state.clone(), Pid::new(231, 0), Some(parent.pid()), "child".to_string(), &child_props)
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
  assert!(
    parent.mailbox().is_suspended(),
    "report_failure は親 mailbox を suspend しなければならない (既存契約)"
  );

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
  let child =
    ActorCell::create(state.clone(), Pid::new(241, 0), Some(parent.pid()), "child".to_string(), &child_props)
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
  assert_eq!(
    snapshot,
    vec!["pre_start"],
    "AC-H3 × MB-H1: suspended 中の子は user message を drain してはならない"
  );
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
fn ac_h2_t2_fault_terminate_with_children_transitions_to_terminating() {
  // AC-H2: handle_stop (fault_terminate) は live child がある間 post_stop と
  // mark_terminated を遅延し、ChildrenContainer を Terminating(Termination) に
  // 遷移させる。Pekko `FaultHandling.scala:terminate` の child stop ループ参照。
  // 観測: post_stop が log に積まれていないこと + children_state_is_terminating。
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
  assert!(
    !parent.children_state_is_normal(),
    "AC-H2: Terminating 中は is_normal=false"
  );
}

#[test]
fn ac_h2_t3_finish_terminate_runs_post_stop_after_last_child() {
  // AC-H2: Terminating(Termination) 状態で最後の子が handle_terminated
  // されると finish_terminate が起動し、post_stop が実行される。
  // Pekko `FaultHandling.scala:finishTerminate` の終端遷移を観測する。
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

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");
  parent_invoker.system_invoke(SystemMessage::Stop).expect("parent stop with live child");
  assert_eq!(log.lock().clone(), vec!["pre_start"], "事前条件: post_stop は遅延されている");

  // 最後の子が terminated → finish_terminate 経路で post_stop が完了する。
  parent.handle_terminated(child.pid()).expect("handle_terminated");

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
//   4. そうでなければ ChildrenContainer を Recreation(cause) で suspend し、
//      最後の子が handle_terminated されたタイミングで finishRecreate を遅延実行
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
    vec![
      "pre_start".to_string(),
      "pre_restart:ac-h4-t1-cause".to_string(),
      "post_restart:ac-h4-t1-cause".to_string(),
    ],
    "AC-H4: 子なし fault_recreate は pre_restart → post_restart を即座に完走させる"
  );
  assert!(
    !cell.mailbox().is_suspended(),
    "AC-H4: finishRecreate 完了時に mailbox は resume されていなければならない"
  );
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
  assert!(
    parent.mailbox().is_suspended(),
    "AC-H4: finishRecreate が遅延されている間 mailbox は suspended のまま"
  );
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
  parent.handle_terminated(child.pid()).expect("handle_terminated");

  let snapshot = log.lock().clone();
  assert_eq!(
    snapshot,
    vec![
      "pre_start".to_string(),
      "pre_restart:ac-h4-t3-cause".to_string(),
      "post_restart:ac-h4-t3-cause".to_string(),
    ],
    "AC-H4: 最後の子 termination 後に finishRecreate(cause) が起動し post_restart が完了する"
  );
  assert!(
    parent.children_state_is_normal(),
    "AC-H4: finishRecreate 後 ChildrenContainer は Normal/Empty に戻る"
  );
  assert!(
    !parent.mailbox().is_suspended(),
    "AC-H4: finishRecreate 完了時に mailbox は resume される"
  );
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
  assert!(
    cell.is_failed_fatally(),
    "AC-H4: fatally 状態は fault_recreate (no-op) を経ても維持される"
  );
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
  assert!(
    !cell.mailbox().is_suspended(),
    "AL-H1: finishRecreate 完了時に mailbox は resume されていなければならない"
  );
}

#[test]
fn al_h1_t2_default_pre_restart_stops_children_and_defers_finish_recreate() {
  // AL-H1: 既定 pre_restart は stop_all_children を呼ぶことで子を terminate
  // キューに乗せた後、自身の post_stop を呼ぶ。childrenRefs は live child を
  // 残したまま Terminating(Recreation) へ遷移するため finishRecreate は遅延され、
  // 子が handle_terminated されるタイミングで post_restart (= 既定の pre_start
  // 委譲) が走る。
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let parent_props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let parent = ActorCell::create(state.clone(), Pid::new(810, 0), None, "al-h1-t2".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child = ActorCell::create(
    state.clone(),
    Pid::new(811, 0),
    Some(parent.pid()),
    "al-h1-t2-child".to_string(),
    &child_props,
  )
  .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());

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
  assert!(
    !parent.children_state_is_normal(),
    "AL-H1: Terminating 中は is_normal=false"
  );

  // 最後の子が terminated → finishRecreate → recreate_actor → 既定 post_restart
  // → 既定 pre_start 委譲、の順序で続きが走る。
  parent.handle_terminated(child.pid()).expect("handle_terminated");

  let final_snapshot = log.lock().clone();
  assert_eq!(
    final_snapshot,
    vec!["pre_start", "post_stop", "pre_start"],
    "AL-H1: 子 termination 後に finishRecreate 経由で 既定 post_restart が pre_start を呼ぶ"
  );
  assert!(
    parent.children().is_empty(),
    "AL-H1: finishRecreate 後は children() は空"
  );
  assert!(
    !parent.mailbox().is_suspended(),
    "AL-H1: finishRecreate 完了後は mailbox を resume"
  );
  assert!(
    parent.children_state_is_normal(),
    "AL-H1: finishRecreate 後は ChildrenContainer が Normal に戻る"
  );
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
  let child = ActorCell::create(
    state.clone(),
    Pid::new(821, 0),
    Some(parent.pid()),
    "al-h1-t3-child".to_string(),
    &child_props,
  )
  .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");
  assert_eq!(log.lock().clone(), vec!["pre_start".to_string()]);

  parent.mailbox().suspend();
  let cause = ActorErrorReason::new("al-h1-t3-cause");
  parent_invoker.system_invoke(SystemMessage::Recreate(cause)).expect("recreate");

  // override pre_restart は children を stop しないため、children() に child が残り、
  // post_stop も呼ばれない。kernel は live child があるため finishRecreate を遅延する。
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
//   - `terminatedQueued: HashSet[ActorRef]` ── DeathWatchNotification を受けた後、
//      user queue に Terminated を投入済み (= 重複投入を抑止) のマーカー
//   - `watchedActorTerminated(actor)` ── DeathWatchNotification ハンドラ:
//        if (watching.contains(actor) && !isTerminating)
//          self.tell(Terminated(actor)); terminatedQueued += actor
//
// fraktor-rs では:
//   - `SystemMessage::DeathWatchNotification(Pid)` を kernel 内通知に使う
//   - watcher 側は `state.watching` (新設) と `state.terminated_queued` (新設) で
//     dedup し、user-level `Terminated` を user queue へ投入する
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

  assert!(
    cell.terminated_queued().is_empty(),
    "AC-H5: 新規 cell は terminated_queued を持たない"
  );
  assert!(
    !cell.is_watching(Pid::new(501, 0)),
    "AC-H5: 新規 cell は何も watch していない"
  );
}

#[test]
fn ac_h5_t2_death_watch_notification_adds_target_to_terminated_queued_when_watching() {
  // AC-H5: Pekko `watchedActorTerminated` の主経路:
  //   1. watcher が target を watching に登録済み
  //   2. DeathWatchNotification(target) が watcher の system queue へ届く
  //   3. watcher は `terminated_queued += target` を行い、user queue へ
  //      Terminated(target) を投入する
  // 本テストは (1)(2) のみ駆動し、kernel が `terminated_queued` を更新することを
  // 確認する (user queue 投入の中身は user-level dispatch の責務で別経路)。
  let state = ActorSystem::new_empty().state();
  let watcher_props = Props::from_fn(|| ProbeActor);
  let watcher = ActorCell::create(state.clone(), Pid::new(510, 0), None, "h5-watcher".to_string(), &watcher_props)
    .expect("create watcher");
  state.register_cell(watcher.clone());

  let target_pid = Pid::new(511, 0);
  watcher.register_watching(target_pid);
  assert!(watcher.is_watching(target_pid), "AC-H5: register_watching で watching set に入る");

  let mut invoker = ActorCellInvoker { cell: watcher.downgrade() };
  invoker
    .system_invoke(SystemMessage::DeathWatchNotification(target_pid))
    .expect("death-watch-notification");

  assert_eq!(
    watcher.terminated_queued(),
    vec![target_pid],
    "AC-H5: watching 中の target に対する DeathWatchNotification は terminated_queued に積まれる"
  );
}

#[test]
fn ac_h5_t3_duplicate_death_watch_notifications_dedupe_via_terminated_queued() {
  // AC-H5: Pekko `terminatedQueueWatchedActor` の dedup 契約:
  //   `if (terminatedQueued contains subject) ()` ── 既に積んでいるなら no-op。
  // 同じ pid に対する DeathWatchNotification を 2 回送っても、`terminated_queued`
  // は単一エントリで終わる。これにより user queue に Terminated が複数投入される
  // 二重配送を防ぐ。
  let state = ActorSystem::new_empty().state();
  let watcher_props = Props::from_fn(|| ProbeActor);
  let watcher = ActorCell::create(state.clone(), Pid::new(520, 0), None, "h5-dedup".to_string(), &watcher_props)
    .expect("create watcher");
  state.register_cell(watcher.clone());

  let target_pid = Pid::new(521, 0);
  watcher.register_watching(target_pid);

  let mut invoker = ActorCellInvoker { cell: watcher.downgrade() };
  invoker
    .system_invoke(SystemMessage::DeathWatchNotification(target_pid))
    .expect("dwn-1");
  invoker
    .system_invoke(SystemMessage::DeathWatchNotification(target_pid))
    .expect("dwn-2");

  let queued = watcher.terminated_queued();
  assert_eq!(
    queued,
    vec![target_pid],
    "AC-H5: 重複した DeathWatchNotification は terminated_queued で dedup される"
  );
  assert_eq!(
    queued.len(),
    1,
    "AC-H5: 同一 pid のエントリが複数積まれてはならない (user queue 二重配送防止)"
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
  invoker
    .system_invoke(SystemMessage::DeathWatchNotification(stranger_pid))
    .expect("dwn-stranger");

  assert!(
    watcher.terminated_queued().is_empty(),
    "AC-H5: watch していない pid からの DeathWatchNotification は terminated_queued に入らない"
  );
  assert!(
    !watcher.is_watching(stranger_pid),
    "AC-H5: dwn 受信が watching set を変えてはならない"
  );
}

#[test]
fn ac_h5_t5_unwatch_clears_pending_terminated_queued_entry() {
  // AC-H5: Pekko `unwatch` は watching と terminatedQueued の両方から target を
  // 取り除く。これにより、後続の Terminated 配送が user queue に流れても
  // receivedTerminated() の `terminatedQueued contains` 判定が false になり、
  // ユーザー receive まで到達しない。
  let state = ActorSystem::new_empty().state();
  let watcher_props = Props::from_fn(|| ProbeActor);
  let watcher = ActorCell::create(state.clone(), Pid::new(540, 0), None, "h5-unwatch".to_string(), &watcher_props)
    .expect("create watcher");
  state.register_cell(watcher.clone());

  let target_pid = Pid::new(541, 0);
  watcher.register_watching(target_pid);

  let mut invoker = ActorCellInvoker { cell: watcher.downgrade() };
  invoker
    .system_invoke(SystemMessage::DeathWatchNotification(target_pid))
    .expect("dwn");
  assert_eq!(
    watcher.terminated_queued(),
    vec![target_pid],
    "事前条件: terminated_queued に target が積まれている"
  );

  watcher.unregister_watching(target_pid);

  assert!(
    !watcher.is_watching(target_pid),
    "AC-H5: unregister_watching で watching set から外れる"
  );
  assert!(
    watcher.terminated_queued().is_empty(),
    "AC-H5: unregister_watching は terminated_queued の保留エントリも掃除する"
  );
}

#[test]
fn ac_h5_t6_terminated_queued_cleared_after_user_terminated_dispatched() {
  // AC-H5: Pekko `receivedTerminated`:
  //   if (terminatedQueued contains t.actor) {
  //     terminatedQueued -= t.actor
  //     receiveMessage(t)
  //   }
  // user queue から `Terminated(target)` を取り出して on_terminated に dispatch
  // した時点で terminated_queued から target が消える。AC-H5 ではこの責務を
  // `cell.handle_terminated(target)` (user queue 経路) が担う。
  let state = ActorSystem::new_empty().state();
  let watcher_props = Props::from_fn(|| ProbeActor);
  let watcher = ActorCell::create(state.clone(), Pid::new(550, 0), None, "h5-clear".to_string(), &watcher_props)
    .expect("create watcher");
  state.register_cell(watcher.clone());

  let target_pid = Pid::new(551, 0);
  watcher.register_watching(target_pid);

  let mut invoker = ActorCellInvoker { cell: watcher.downgrade() };
  invoker
    .system_invoke(SystemMessage::DeathWatchNotification(target_pid))
    .expect("dwn");
  assert_eq!(watcher.terminated_queued(), vec![target_pid]);

  // user queue 経由で Terminated(target) が処理されたことを simulate する:
  // handle_terminated は AC-H5 拡張で「terminated_queued から target を消す」
  // 責務を獲得する。
  watcher.handle_terminated(target_pid).expect("handle_terminated");

  assert!(
    watcher.terminated_queued().is_empty(),
    "AC-H5: handle_terminated 後は terminated_queued から target が除去される"
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
