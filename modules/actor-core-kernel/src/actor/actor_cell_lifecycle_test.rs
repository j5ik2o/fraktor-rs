use super::*;

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
