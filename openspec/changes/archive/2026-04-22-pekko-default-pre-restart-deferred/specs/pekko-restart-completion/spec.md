## ADDED Requirements

### Requirement: default pre_restart + 子あり restart は同期 dispatcher 上でも deferred されなければならない

`Actor::pre_restart` の既定実装（`ctx.stop_all_children()` + `self.post_stop(ctx)`）を持つ actor が `SystemMessage::Recreate(cause)` を受信したとき、child が存在すれば `finish_recreate` の実行は最後の child からの `SystemMessage::DeathWatchNotification(child)` 受信まで deferred されなければならない（MUST）。この deferred 契約は production の async dispatcher 上だけでなく、同期 / inline dispatcher 上でも成立しなければならない（MUST）。

この要件は `fault_recreate` 内の `pre_restart` 呼び出しを `MessageDispatcherShared::run_with_drive_guard` でラップし、既存 `ExecutorShared` トランポリン
(`executor_shared.rs:40-146`) の `running: ArcShared<AtomicBool>` を外部から CAS で claim する
ことで達成される。guard 中は `send_system_message(child, Stop)` が起動する `ExecutorShared::execute`
が `running = true` を観測して `trampoline.pending` に task を push するだけで return し、
同一 thread 上での child mailbox の inline drain（および parent への DWN 再入）が発生しない。
production dispatcher は既にこのトランポリンを worker thread 経由で利用しているため、guard の有無に
よらず挙動は変化しない。

#### Scenario: default pre_restart + 子あり restart は Recreate 処理中に finish_recreate を起動しない

- **GIVEN** parent actor が `Actor::pre_restart` の既定実装を使用しており、child 1 件以上を
  `children_state` に保持している状態
- **AND** parent の mailbox が `suspend()` 済み
- **WHEN** `ActorCellInvoker::system_invoke(SystemMessage::Recreate(cause))` が呼ばれる
- **THEN** `fault_recreate` → 既定 `pre_restart` → `ctx.stop_all_children()` + `self.post_stop(ctx)`
  が実行される
- **AND** `set_children_termination_reason(SuspendReason::Recreation(cause))` が `true` を返し
  deferred=true で return する
- **AND** `system_invoke` から制御が戻った直後の時点で、`actor.post_restart` はまだ呼ばれておらず、
  `children_state` は `Terminating{Recreation(cause)}` 状態で child が `to_die` に残ったままである

#### Scenario: 最後の child の DeathWatchNotification が finish_recreate を駆動する（sync dispatch 上）

- **GIVEN** 前 scenario の終状態（parent が deferred、child が Terminating{Recreation} の to_die に残存）
- **WHEN** 明示的に `parent.handle_death_watch_notification(child_pid)` が呼ばれる（production では
  dispatcher が child の Stop を実行後 parent へ DWN を送ることで自然に起動、test では直接 simulate）
- **THEN** `remove_child_and_get_state_change(child_pid)` が `Some(SuspendReason::Recreation(cause))`
  を返す
- **AND** `finish_recreate(cause)` が駆動され、既定 `post_restart` 経由で `pre_start` が呼ばれる
- **AND** `parent.children_state_is_normal()` が真に戻る
- **AND** `parent.mailbox().is_suspended()` が偽に戻る

#### Scenario: default pre_restart は複数 child の全停止まで finish_recreate を deferred する

- **GIVEN** parent actor が既定 `pre_restart` を使用しており、child A / B の 2 件を保持している状態
- **WHEN** `SystemMessage::Recreate(cause)` が parent に届く
- **THEN** `fault_recreate` 完了時点で `children_state` は `Terminating{Recreation(cause), to_die=[A, B]}`
- **AND** child A の `handle_death_watch_notification(A)` が最初に届いても、`remove_child_and_get_state_change`
  は `None` を返し `finish_recreate` は駆動されない
- **AND** 続いて child B の `handle_death_watch_notification(B)` が届いたときに初めて
  `Some(SuspendReason::Recreation(cause))` が返り、`finish_recreate` が駆動される
