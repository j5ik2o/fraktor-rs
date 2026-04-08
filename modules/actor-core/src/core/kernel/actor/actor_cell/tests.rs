use alloc::{string::ToString, vec, vec::Vec};
use core::{hint::spin_loop, num::NonZeroUsize};

use fraktor_utils_core_rs::core::sync::{ArcShared, NoStdMutex};

use super::ActorCell;
use crate::core::kernel::{
  actor::{
    Actor, ActorContext, Pid,
    error::ActorError,
    messaging::{
      ActorIdentity, AnyMessage, AnyMessageView, Identify, message_invoker::MessageInvoker,
      system_message::SystemMessage,
    },
    props::{MailboxConfig, Props},
    supervision::{SupervisorDirective, SupervisorStrategy, SupervisorStrategyConfig, SupervisorStrategyKind},
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
  log: ArcShared<NoStdMutex<Vec<Pid>>>,
}

impl RecordingActor {
  fn new(log: ArcShared<NoStdMutex<Vec<Pid>>>) -> Self {
    Self { log }
  }
}

struct LifecycleRecorderActor {
  log: ArcShared<NoStdMutex<Vec<&'static str>>>,
}

impl LifecycleRecorderActor {
  fn new(log: ArcShared<NoStdMutex<Vec<&'static str>>>) -> Self {
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
  received: ArcShared<NoStdMutex<Vec<i32>>>,
}

impl OrderedMessageActor {
  fn new(received: ArcShared<NoStdMutex<Vec<i32>>>) -> Self {
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
  received: ArcShared<NoStdMutex<usize>>,
  replies:  ArcShared<NoStdMutex<Vec<ActorIdentity>>>,
}

impl IdentityProbeActor {
  fn new(received: ArcShared<NoStdMutex<usize>>, replies: ArcShared<NoStdMutex<Vec<ActorIdentity>>>) -> Self {
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
    ctx.set_receive_timeout(core::time::Duration::from_millis(20), AnyMessage::new("timeout"));
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
    SupervisorStrategy::new(SupervisorStrategyKind::OneForOne, 1, core::time::Duration::from_secs(1), |_| {
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
#[should_panic(expected = "ActorCell::install_mailbox called twice for the same cell")]
fn actor_cell_install_mailbox_panics_on_double_install() {
  let system = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(system.clone(), Pid::new(7, 0), None, "double-install".to_string(), &props).expect("cell");
  // The cell already installed its mailbox during `create`. Re-installing it
  // must trip the debug-build assertion that enforces the install-once
  // contract.
  let mailbox = cell.mailbox();
  cell.install_mailbox(mailbox);
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
  let second = mailbox.enqueue_user(AnyMessage::new(2_u32));
  assert!(matches!(second, Err(_)), "DropNewest should reject the second enqueue past capacity 1");
  assert_eq!(mailbox.user_len(), 1);
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
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
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
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(40, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = super::ActorCellInvoker { cell: cell.downgrade() };
  invoker.invoke_system_message(SystemMessage::Create).expect("create");

  let snapshot = log.lock().clone();
  assert_eq!(snapshot, vec!["pre_start"]);
}

#[test]
fn identify_replies_with_actor_identity_without_invoking_actor() {
  let system = ActorSystem::new_empty().state();
  let actor_received = ArcShared::new(NoStdMutex::new(0usize));
  let actor_replies = ArcShared::new(NoStdMutex::new(Vec::new()));
  let actor_props = Props::from_fn({
    let actor_received = actor_received.clone();
    let actor_replies = actor_replies.clone();
    move || IdentityProbeActor::new(actor_received.clone(), actor_replies.clone())
  });
  let target =
    ActorCell::create(system.clone(), Pid::new(60, 0), None, "target".to_string(), &actor_props).expect("target");
  let reply_received = ArcShared::new(NoStdMutex::new(0usize));
  let reply_replies = ArcShared::new(NoStdMutex::new(Vec::new()));
  let reply_props = Props::from_fn({
    let reply_received = reply_received.clone();
    let reply_replies = reply_replies.clone();
    move || IdentityProbeActor::new(reply_received.clone(), reply_replies.clone())
  });
  let reply_to =
    ActorCell::create(system.clone(), Pid::new(61, 0), None, "reply".to_string(), &reply_props).expect("reply");
  system.register_cell(target.clone());
  system.register_cell(reply_to.clone());

  let mut invoker = super::ActorCellInvoker { cell: target.downgrade() };
  let identify = Identify::new(AnyMessage::new("corr"));
  let message = AnyMessage::new(identify).with_sender(reply_to.actor_ref());

  invoker.invoke_user_message(message).expect("identify");

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
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(41, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = super::ActorCellInvoker { cell: cell.downgrade() };
  invoker.invoke_system_message(SystemMessage::Create).expect("create");
  invoker.invoke_system_message(SystemMessage::Recreate).expect("recreate");

  let snapshot = log.lock().clone();
  assert_eq!(snapshot, vec!["pre_start", "post_stop", "pre_start"]);
}

#[test]
fn poison_pill_system_message_invokes_post_stop() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(410, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = super::ActorCellInvoker { cell: cell.downgrade() };
  invoker.invoke_system_message(SystemMessage::Create).expect("create");
  invoker.invoke_system_message(SystemMessage::PoisonPill).expect("poison pill");

  let snapshot = log.lock().clone();
  assert_eq!(snapshot, vec!["pre_start", "post_stop"]);
}

#[test]
fn kill_system_message_reports_fatal_failure() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(411, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = super::ActorCellInvoker { cell: cell.downgrade() };
  invoker.invoke_system_message(SystemMessage::Create).expect("create");
  let error = invoker.invoke_system_message(SystemMessage::Kill).expect_err("kill should report failure");

  assert_eq!(error, ActorError::fatal("Kill"));
}

#[test]
fn poison_pill_user_message_preserves_user_ordering() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(412, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = super::ActorCellInvoker { cell: cell.downgrade() };
  invoker.invoke_system_message(SystemMessage::Create).expect("create");

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
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(413, 0), None, "probe".to_string(), &props).expect("create actor cell");
  state.register_cell(cell.clone());

  let mut invoker = super::ActorCellInvoker { cell: cell.downgrade() };
  invoker.invoke_system_message(SystemMessage::Create).expect("create");
  let error = invoker.invoke_user_message(AnyMessage::new(SystemMessage::Kill)).expect_err("kill should fail");
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

  let mut parent_invoker = super::ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.invoke_system_message(SystemMessage::Create).expect("create parent");

  let mut invoker = super::ActorCellInvoker { cell: cell.downgrade() };
  invoker.invoke_system_message(SystemMessage::Create).expect("create");

  let initial_handle = cell
    .receive_timeout
    .lock()
    .as_ref()
    .and_then(super::super::actor_context::ReceiveTimeoutState::handle_raw)
    .expect("receive timeout handle should exist after pre_start");

  let error = invoker.invoke_user_message(AnyMessage::new(1_u32)).expect_err("user message should fail");
  assert_eq!(error, ActorError::recoverable("boom"));

  let current_handle = cell
    .receive_timeout
    .lock()
    .as_ref()
    .and_then(super::super::actor_context::ReceiveTimeoutState::handle_raw)
    .expect("receive timeout handle should remain registered after failure");

  assert_eq!(current_handle, initial_handle, "failure path must not arm a fresh receive-timeout timer");
}

#[test]
fn system_queue_is_drained_before_user_queue() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
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
  let received = ArcShared::new(NoStdMutex::new(Vec::new()));
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

  cell.state.lock().stashed_messages.push_back(AnyMessage::new(1_i32));

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

  cell.state.lock().stashed_messages.push_back(AnyMessage::new(1_i32));
  cell.state.lock().stashed_messages.push_back(AnyMessage::new(2_i32));

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
  let log = ArcShared::new(NoStdMutex::new(Vec::new()));
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
