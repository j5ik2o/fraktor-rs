use crate::{
  actor::{actor_cell::tests::*, actor_cell_dispatch::ActorCellInvoker, messaging::message_invoker::MessageInvoker},
  system::guardian::GuardianKind,
};

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
fn stopping_root_guardian_marks_system_terminated() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let root =
    ActorCell::create(state.clone(), Pid::new(44, 0), None, "root-guardian".to_string(), &props).expect("root");
  state.register_cell(root.clone());
  state.set_root_guardian(&root);
  let mut invoker = ActorCellInvoker { cell: root.downgrade() };

  invoker.system_invoke(SystemMessage::Stop).expect("stop root guardian");

  assert!(state.is_terminated(), "root guardian stop should terminate the system");
}

#[test]
fn stopping_user_guardian_after_root_has_stopped_marks_system_terminated() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| ProbeActor);
  let root =
    ActorCell::create(state.clone(), Pid::new(45, 0), None, "root-guardian".to_string(), &props).expect("root");
  let user = ActorCell::create(state.clone(), Pid::new(46, 0), Some(root.pid()), "user-guardian".to_string(), &props)
    .expect("user");
  state.register_cell(root.clone());
  state.register_cell(user.clone());
  state.set_root_guardian(&root);
  state.set_user_guardian(&user);
  state.mark_guardian_stopped(GuardianKind::Root);
  let mut invoker = ActorCellInvoker { cell: user.downgrade() };

  invoker.system_invoke(SystemMessage::Stop).expect("stop user guardian");

  assert!(state.is_terminated(), "user guardian stop after root stop should terminate the system");
}
