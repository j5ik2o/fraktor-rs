use super::*;

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
