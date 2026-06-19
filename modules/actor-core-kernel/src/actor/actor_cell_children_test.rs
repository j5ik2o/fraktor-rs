use super::*;

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
