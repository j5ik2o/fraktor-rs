use super::*;
use crate::{
  actor::{
    actor_ref::{ActorRef, ActorRefSender, SendOutcome},
    error::SendError,
  },
  dispatch::mailbox::metrics_event::MailboxPressureEvent,
};

struct FailingReplySender;

impl ActorRefSender for FailingReplySender {
  fn send(&mut self, message: AnyMessage) -> Result<SendOutcome, SendError> {
    Err(SendError::closed(message))
  }
}

struct MailboxPressureProbeActor {
  calls: ArcShared<SpinSyncMutex<Vec<u8>>>,
  fail:  bool,
}

impl MailboxPressureProbeActor {
  fn new(calls: ArcShared<SpinSyncMutex<Vec<u8>>>, fail: bool) -> Self {
    Self { calls, fail }
  }
}

impl Actor for MailboxPressureProbeActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn on_mailbox_pressure(
    &mut self,
    _ctx: &mut ActorContext<'_>,
    event: &MailboxPressureEvent,
  ) -> Result<(), ActorError> {
    self.calls.lock().push(event.utilization());
    if self.fail { Err(ActorError::recoverable("mailbox pressure failed")) } else { Ok(()) }
  }
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
fn identify_without_sender_is_dropped_without_invoking_actor() {
  let system = ActorSystem::new_empty().state();
  let actor_received = ArcShared::new(SpinSyncMutex::new(0usize));
  let actor_replies = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let actor_props = Props::from_fn({
    let actor_received = actor_received.clone();
    let actor_replies = actor_replies.clone();
    move || IdentityProbeActor::new(actor_received.clone(), actor_replies.clone())
  });
  let target = ActorCell::create(system.clone(), Pid::new(62, 0), None, "target-no-sender".to_string(), &actor_props)
    .expect("target");
  system.register_cell(target.clone());

  let mut invoker = ActorCellInvoker { cell: target.downgrade() };
  invoker.invoke(AnyMessage::new(Identify::new(AnyMessage::new("corr")))).expect("identify");

  assert_eq!(*actor_received.lock(), 0, "identify should not reach the actor receive method");
  assert!(actor_replies.lock().is_empty(), "identify without sender should not produce a reply");
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
fn dropped_actor_cell_invoker_ignores_user_system_and_pressure_messages() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(430, 0), None, "dropped".to_string(), &props).expect("create actor cell");
  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  let event = MailboxPressureEvent::new(Pid::new(430, 0), 8, 10, 80, Duration::from_millis(1), Some(7));

  drop(cell);

  invoker.invoke(AnyMessage::new(1_i32)).expect("dropped user invoke is ignored");
  invoker.system_invoke(SystemMessage::Create).expect("dropped system invoke is ignored");
  invoker.invoke_mailbox_pressure(&event).expect("dropped pressure invoke is ignored");
}

#[test]
fn terminated_actor_cell_invoker_ignores_user_and_system_messages() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell = ActorCell::create(state.clone(), Pid::new(431, 0), None, "terminated".to_string(), &props)
    .expect("create actor cell");
  state.register_cell(cell.clone());
  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  cell.mark_terminated();

  invoker.invoke(AnyMessage::new(())).expect("terminated user invoke is ignored");
  invoker.system_invoke(SystemMessage::Stop).expect("terminated system invoke is ignored");

  assert_eq!(log.lock().clone(), vec!["pre_start"]);
}

#[test]
fn non_auto_receive_system_message_payload_is_delivered_as_user_message() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(432, 0), None, "system-payload".to_string(), &props).expect("cell");
  state.register_cell(cell.clone());
  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");

  invoker.invoke(AnyMessage::new(SystemMessage::Create)).expect("user payload");

  assert_eq!(log.lock().clone(), vec!["pre_start", "receive"]);
}

#[test]
fn identify_reply_send_error_is_reported_to_caller() {
  let state = ActorSystem::new_empty().state();
  let target_props = Props::from_fn(|| ProbeActor);
  let target =
    ActorCell::create(state.clone(), Pid::new(433, 0), None, "target".to_string(), &target_props).expect("target");
  state.register_cell(target.clone());
  let reply_to = ActorRef::new_with_builtin_lock(Pid::new(434, 0), FailingReplySender);
  let identify = Identify::new(AnyMessage::new("closed-reply"));
  let message = AnyMessage::new(identify).with_sender(reply_to);
  let mut invoker = ActorCellInvoker { cell: target.downgrade() };

  let result = invoker.invoke(message);

  assert!(result.is_err(), "closed reply mailbox should surface send error");
}

#[test]
fn mailbox_pressure_invokes_actor_hook() {
  let state = ActorSystem::new_empty().state();
  let calls = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let calls = calls.clone();
    move || MailboxPressureProbeActor::new(calls.clone(), false)
  });
  let cell = ActorCell::create(state.clone(), Pid::new(435, 0), None, "pressure".to_string(), &props).expect("cell");
  state.register_cell(cell.clone());
  let event = MailboxPressureEvent::new(cell.pid(), 9, 10, 90, Duration::from_millis(1), Some(8));
  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };

  invoker.invoke(AnyMessage::new(())).expect("user receive");
  invoker.invoke_mailbox_pressure(&event).expect("pressure hook");

  assert_eq!(calls.lock().clone(), vec![90]);
}

#[test]
fn mailbox_pressure_failure_reports_actor_failure() {
  let state = ActorSystem::new_empty().state();
  let calls = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let props = Props::from_fn({
    let calls = calls.clone();
    move || MailboxPressureProbeActor::new(calls.clone(), true)
  });
  let cell =
    ActorCell::create(state.clone(), Pid::new(436, 0), None, "pressure-fail".to_string(), &props).expect("cell");
  state.register_cell(cell.clone());
  let event = MailboxPressureEvent::new(cell.pid(), 10, 10, 100, Duration::from_millis(1), Some(8));
  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };

  let error = invoker.invoke_mailbox_pressure(&event).expect_err("pressure hook failure");

  assert_eq!(error, ActorError::recoverable("mailbox pressure failed"));
  assert!(cell.is_failed(), "pressure hook errors should be routed through report_failure");
  assert_eq!(calls.lock().clone(), vec![100]);
}
