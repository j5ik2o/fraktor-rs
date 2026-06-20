use super::*;
use crate::actor::messaging::system_message::FailurePayload;

struct PostRestartFailingActor;

impl Actor for PostRestartFailingActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn post_restart(&mut self, _ctx: &mut ActorContext<'_>, _reason: &ActorErrorReason) -> Result<(), ActorError> {
    Err(ActorError::recoverable("post_restart failed"))
  }
}

struct DirectiveSupervisorActor {
  directive:            SupervisorDirective,
  kind:                 SupervisorStrategyKind,
  fail_on_child_failed: bool,
}

impl DirectiveSupervisorActor {
  const fn new(directive: SupervisorDirective, kind: SupervisorStrategyKind) -> Self {
    Self { directive, kind, fail_on_child_failed: false }
  }

  const fn with_child_failed_error(directive: SupervisorDirective, kind: SupervisorStrategyKind) -> Self {
    Self { directive, kind, fail_on_child_failed: true }
  }
}

impl Actor for DirectiveSupervisorActor {
  fn receive(&mut self, _ctx: &mut ActorContext<'_>, _message: AnyMessageView<'_>) -> Result<(), ActorError> {
    Ok(())
  }

  fn supervisor_strategy(&self, _ctx: &mut ActorContext<'_>) -> SupervisorStrategyConfig {
    let directive = self.directive;
    SupervisorStrategy::with_decider(move |_| directive).with_kind(self.kind).into()
  }

  fn on_child_failed(
    &mut self,
    _ctx: &mut ActorContext<'_>,
    _child: Pid,
    _error: &ActorError,
  ) -> Result<(), ActorError> {
    if self.fail_on_child_failed { Err(ActorError::recoverable("on_child_failed failed")) } else { Ok(()) }
  }
}

fn child_failure_payload(child: Pid, reason: &'static str) -> FailurePayload {
  FailurePayload::from_error(child, &ActorError::recoverable(reason), None, Duration::from_millis(1))
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
  parent_invoker.invoke(AnyMessage::new(())).expect("non-failing user message");

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
  // AC-H2: handle_stop (fault_terminate) は live child がある間
  // post_stop と mark_terminated を遅延し、ChildrenContainer を Terminating(Termination)
  // に遷移させる。
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
  assert!(parent.mailbox().is_suspended(), "AC-H2: 子 stop 待ちの間は user mailbox を suspend");

  parent_invoker.system_invoke(SystemMessage::Stop).expect("duplicate parent stop while terminating");
  assert_eq!(
    log.lock().clone(),
    vec!["pre_start"],
    "AC-H2: duplicate Stop while Terminating must keep post_stop deferred"
  );
}

#[test]
fn ac_h2_t3_finish_terminate_runs_post_stop_after_last_child() {
  // AC-H2: Terminating(Termination) 状態で最後の子が
  // handle_death_watch_notification されたとき finish_terminate が起動し、
  // post_stop が実行される。
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

#[test]
fn ac_h2_t4_finish_terminate_ignores_duplicate_child_notification() {
  let state = ActorSystem::new_empty().state();
  let log = ArcShared::new(SpinSyncMutex::new(Vec::new()));
  let parent_props = Props::from_fn({
    let log = log.clone();
    move || LifecycleRecorderActor::new(log.clone())
  });
  let parent = ActorCell::create(state.clone(), Pid::new(276, 0), None, "term-parent3".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child =
    ActorCell::create(state.clone(), Pid::new(277, 0), Some(parent.pid()), "term-child3".to_string(), &child_props)
      .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());
  parent.register_watching(child.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Create).expect("parent create");
  parent_invoker.system_invoke(SystemMessage::Stop).expect("parent stop with live child");

  parent.handle_death_watch_notification(child.pid()).expect("first death-watch notification");
  parent.handle_death_watch_notification(child.pid()).expect("duplicate death-watch notification");

  assert_eq!(
    log.lock().clone(),
    vec!["pre_start", "post_stop"],
    "AC-H2: duplicate child notification must not run post_stop twice"
  );
}

#[test]
fn ac_h2_t5_user_request_child_stop_does_not_finish_parent_terminate() {
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(278, 0), None, "term-parent4".to_string(), &parent_props)
    .expect("create parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child =
    ActorCell::create(state.clone(), Pid::new(279, 0), Some(parent.pid()), "term-child4".to_string(), &child_props)
      .expect("create child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());
  parent.register_supervision_watching(child.pid());

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::StopChild(child.pid())).expect("stop child");
  assert!(
    parent.children_state_is_terminating(),
    "AC-H2: StopChild marks the child container as Terminating(UserRequest)"
  );

  parent.handle_death_watch_notification(child.pid()).expect("child death-watch notification");

  assert!(parent.children().is_empty(), "AC-H2: child is removed after UserRequest completion");
  assert!(
    !parent.children_state_is_terminating(),
    "AC-H2: UserRequest completion returns the child container to normal"
  );
  assert!(state.cell(&parent.pid()).is_some(), "AC-H2: UserRequest completion must not terminate the parent");
}

#[test]
fn ac_h2_t6_fault_terminate_records_child_stop_send_error() {
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| ProbeActor);
  let parent = ActorCell::create(state.clone(), Pid::new(280, 0), None, "term-parent5".to_string(), &parent_props)
    .expect("create parent");
  state.register_cell(parent.clone());

  let missing_child = Pid::new(281, 0);
  parent.register_child(missing_child);

  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Stop).expect("parent stop with missing child");

  let dead_letters = state.dead_letters();
  assert!(
    dead_letters.iter().any(|entry| {
      entry.recipient() == Some(missing_child)
        && entry.reason() == DeadLetterReason::RecipientUnavailable
        && entry.message().downcast_ref::<SystemMessage>().is_some_and(|message| matches!(message, SystemMessage::Stop))
    }),
    "AC-H2: failed Stop delivery to a child must be recorded as deadletter"
  );
}

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

#[test]
fn finish_recreate_post_restart_error_marks_cell_failed_fatally() {
  let state = ActorSystem::new_empty().state();
  let props = Props::from_fn(|| PostRestartFailingActor);
  let cell =
    ActorCell::create(state.clone(), Pid::new(840, 0), None, "post-restart-fail".to_string(), &props).expect("cell");
  state.register_cell(cell.clone());
  let mut invoker = ActorCellInvoker { cell: cell.downgrade() };
  invoker.system_invoke(SystemMessage::Create).expect("create");
  invoker.invoke(AnyMessage::new(())).expect("normal receive");
  cell.mailbox().suspend();

  let error = invoker
    .system_invoke(SystemMessage::Recreate(ActorErrorReason::new("post-restart-cause")))
    .expect_err("post_restart failure should be returned");

  assert_eq!(error, ActorError::recoverable("post_restart failed"));
  assert!(cell.is_failed_fatally(), "post_restart failure should mark the cell fatally failed");
}

#[test]
fn handle_failure_reports_on_child_failed_error_as_parent_failure() {
  let state = ActorSystem::new_empty().state();
  let parent_props = Props::from_fn(|| {
    DirectiveSupervisorActor::with_child_failed_error(SupervisorDirective::Resume, SupervisorStrategyKind::OneForOne)
  });
  let parent =
    ActorCell::create(state.clone(), Pid::new(841, 0), None, "child-failed-parent".to_string(), &parent_props)
      .expect("parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child = ActorCell::create(
    state.clone(),
    Pid::new(842, 0),
    Some(parent.pid()),
    "child-failed-child".to_string(),
    &child_props,
  )
  .expect("child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());
  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.invoke(AnyMessage::new(())).expect("normal receive");

  parent.handle_failure(&child_failure_payload(child.pid(), "child boom"));

  assert!(parent.is_failed(), "on_child_failed error should be routed through report_failure");
  assert!(parent.mailbox().is_suspended(), "parent failure reporting should suspend the parent mailbox");
}

#[test]
fn all_for_one_restart_suspends_sibling_before_recreate_dispatch() {
  let state = ActorSystem::new_empty().state();
  let parent_props =
    Props::from_fn(|| DirectiveSupervisorActor::new(SupervisorDirective::Restart, SupervisorStrategyKind::AllForOne));
  let parent =
    ActorCell::create(state.clone(), Pid::new(843, 0), None, "all-for-one-parent".to_string(), &parent_props)
      .expect("parent");
  let child_a_props = Props::from_fn(|| ProbeActor);
  let child_a =
    ActorCell::create(state.clone(), Pid::new(844, 0), Some(parent.pid()), "all-for-one-a".to_string(), &child_a_props)
      .expect("child a");
  let child_b_props = Props::from_fn(|| ProbeActor);
  let child_b =
    ActorCell::create(state.clone(), Pid::new(845, 0), Some(parent.pid()), "all-for-one-b".to_string(), &child_b_props)
      .expect("child b");
  state.register_cell(parent.clone());
  state.register_cell(child_a.clone());
  state.register_cell(child_b.clone());
  parent.register_child(child_a.pid());
  parent.register_child(child_b.pid());
  child_a.mailbox().suspend();

  parent.handle_failure(&child_failure_payload(child_a.pid(), "restart all"));

  assert!(state.dead_letters().is_empty(), "registered AllForOne children should receive Recreate without deadletters");
}

#[test]
fn restart_failure_records_parent_failure_when_child_mailbox_is_closed() {
  let state = ActorSystem::new_empty().state();
  let parent_props =
    Props::from_fn(|| DirectiveSupervisorActor::new(SupervisorDirective::Restart, SupervisorStrategyKind::OneForOne));
  let parent =
    ActorCell::create(state.clone(), Pid::new(846, 0), None, "restart-closed-parent".to_string(), &parent_props)
      .expect("parent");
  let missing_child = Pid::new(847, 0);
  state.register_cell(parent.clone());
  parent.register_child(missing_child);

  parent.handle_failure(&child_failure_payload(missing_child, "restart missing"));

  assert!(parent.is_failed(), "failed restart delivery should escalate through parent report_failure");
  assert!(
    state.dead_letters().iter().any(|entry| {
      entry.recipient() == Some(missing_child)
        && entry
          .message()
          .downcast_ref::<SystemMessage>()
          .is_some_and(|message| matches!(message, SystemMessage::Recreate(_)))
    }),
    "failed Recreate delivery should be recorded"
  );
}

#[test]
fn stop_escalate_and_resume_directives_record_closed_child_send_errors() {
  for (index, directive) in
    [SupervisorDirective::Stop, SupervisorDirective::Escalate, SupervisorDirective::Resume].into_iter().enumerate()
  {
    let state = ActorSystem::new_empty().state();
    let parent_pid = Pid::new(850 + index as u64 * 2, 0);
    let missing_child = Pid::new(851 + index as u64 * 2, 0);
    let parent_props =
      Props::from_fn(move || DirectiveSupervisorActor::new(directive, SupervisorStrategyKind::OneForOne));
    let parent = ActorCell::create(state.clone(), parent_pid, None, "directive-parent".to_string(), &parent_props)
      .expect("parent");
    state.register_cell(parent.clone());
    parent.register_child(missing_child);

    parent.handle_failure(&child_failure_payload(missing_child, "directive missing"));

    assert!(
      state.dead_letters().iter().any(|entry| {
        entry.recipient() == Some(missing_child)
          && entry.message().downcast_ref::<SystemMessage>().is_some_and(|message| match directive {
            | SupervisorDirective::Stop | SupervisorDirective::Escalate => matches!(message, SystemMessage::Stop),
            | SupervisorDirective::Resume => matches!(message, SystemMessage::Resume),
            | SupervisorDirective::Restart => unreachable!("restart is covered separately"),
          })
      }),
      "{directive:?} should record closed child send error"
    );
  }
}

#[test]
fn handle_child_failure_on_terminated_container_falls_back_to_stop_with_no_affected_children() {
  let state = ActorSystem::new_empty().state();
  let parent_props =
    Props::from_fn(|| DirectiveSupervisorActor::new(SupervisorDirective::Stop, SupervisorStrategyKind::AllForOne));
  let parent = ActorCell::create(state.clone(), Pid::new(856, 0), None, "terminated-parent".to_string(), &parent_props)
    .expect("parent");
  let child_props = Props::from_fn(|| ProbeActor);
  let child = ActorCell::create(
    state.clone(),
    Pid::new(857, 0),
    Some(parent.pid()),
    "terminated-child".to_string(),
    &child_props,
  )
  .expect("child");
  state.register_cell(parent.clone());
  state.register_cell(child.clone());
  parent.register_child(child.pid());
  parent.register_watching(child.pid());
  let mut parent_invoker = ActorCellInvoker { cell: parent.downgrade() };
  parent_invoker.system_invoke(SystemMessage::Stop).expect("stop parent");
  parent.handle_death_watch_notification(child.pid()).expect("finish terminate");

  let (directive, affected) = parent.handle_child_failure(
    Pid::new(858, 0),
    &ActorError::recoverable("after terminated"),
    Duration::from_millis(1),
  );

  assert_eq!(directive, SupervisorDirective::Stop);
  assert!(affected.is_empty(), "terminated child container should not produce affected children");
}
