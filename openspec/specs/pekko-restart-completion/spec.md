# pekko-restart-completion Specification

## Purpose
TBD - created by archiving change 2026-04-20-pekko-restart-completion. Update Purpose after archive.
## Requirements
### Requirement: Restart は 2 フェーズ state machine で駆動されなければならない

actor の restart は `fault_recreate(cause)` と `finish_recreate(cause)` の 2 フェーズで駆動されなければならない（MUST）。子が存在する場合、`fault_recreate` は `ChildrenContainer` に `SuspendReason::Recreation(cause)` をタグ付けして deferred 状態で return し、最後の子の停止通知を `SystemMessage::DeathWatchNotification(pid)` として受信した `handle_death_watch_notification` が `remove_child_and_get_state_change` 経由で `finish_recreate(cause)` を発火する。

#### Scenario: 子なし restart は同期 fallthrough する

- **WHEN** `ActorCell` が `SystemMessage::Recreate(cause)` を受信する
- **AND** 子 actor が存在しない
- **THEN** `fault_recreate(cause)` は `set_children_termination_reason(Recreation(cause))` が `false` を返すことで `finish_recreate(cause)` に即時フォールスルーする
- **AND** `deferred_recreate_cause` は `None` のまま、または一時的に `Some` になって `finish_recreate` 冒頭で `take()` される

#### Scenario: 子あり restart は子終了まで deferred

- **WHEN** `ActorCell` が `SystemMessage::Recreate(cause)` を受信する
- **AND** 生存中の子 actor が 1 人以上存在する
- **THEN** `fault_recreate(cause)` は `pre_restart(&mut ctx, &cause)` を呼んだ後 `deferred_recreate_cause = Some(cause)` を設定し、`set_children_termination_reason(Recreation(cause))` が `true` を返したタイミングで return する
- **AND** `finish_recreate` はまだ呼ばれない

#### Scenario: 最後の子の DeathWatchNotification が `finish_recreate` を駆動する

- **WHEN** deferred 状態の `ActorCell` の最後の子から `SystemMessage::DeathWatchNotification(pid)` を受信する
- **AND** `handle_death_watch_notification` が `remove_child_and_get_state_change(pid)` で `Some(SuspendReason::Recreation(cause))` を受け取る
- **THEN** `finish_recreate(cause)` が発火される
- **AND** `deferred_recreate_cause` は `finish_recreate` 内で `None` にクリアされる
- **AND** `SystemMessage::Terminated(pid)` は kernel 内から送信されない（唯一の kernel 内 envelope は `DeathWatchNotification`）

#### Scenario: `finish_recreate` は `post_restart` を呼ぶ

- **WHEN** `finish_recreate(cause)` が実行される
- **THEN** `recreate_actor` → `clear_failed` → `mailbox().resume()` → `actor.post_restart(&mut ctx, &cause)` の順で実行される
- **AND** `post_restart` が `Ok(())` なら `publish_lifecycle(LifecycleStage::Restarted)` が発行される
- **AND** `post_restart` が `Err(error)` なら `set_failed_fatally()` + `report_failure(&error, None)` でエスカレーションされる

### Requirement: SystemMessage::Recreate は実 cause を handle_failure から保持しなければならない

`SystemMessage::Recreate` は `ActorErrorReason` を payload として保持しなければならない（MUST）。cause は `handle_child_failure` に入力された `&ActorError` に由来する実原因でなければならず、仮値（例: `ActorErrorReason::new("restart")`）を埋めて送信してはならない（MUST NOT）。supervisor directive `Restart` 経路からの送信と、Restart 中の生存子への再帰送信の双方で実 cause が保持され、最終的に `pre_restart(&mut ctx, &cause)` / `post_restart(&mut ctx, &cause)` まで同一 `ActorErrorReason` が伝播する。

#### Scenario: `handle_failure` は supervisor 経由で実 cause を送信する

- **WHEN** `actor_cell.rs:1145` の `handle_failure` が `SupervisorDirective::Restart` を選択する
- **THEN** affected target への送信は `SystemMessage::Recreate(actor_error.to_reason())` である
- **AND** `actor_error` は `payload.to_actor_error()` 由来の実原因である

#### Scenario: system_state 経路も実 cause を送信する

- **WHEN** `system_state.rs:991` / `system_state_shared.rs:775` の `handle_failure` 分岐が `SupervisorDirective::Restart` を選択する
- **THEN** affected target への送信は `SystemMessage::Recreate(error.to_reason())` である（`error` は `handle_child_failure` に渡された `&ActorError`）
- **AND** 仮値（`ActorErrorReason::new("restart")` 等）は使用されない

#### Scenario: cause は `pre_restart` / `post_restart` まで同一実体として保持される

- **WHEN** `SystemMessage::Recreate(cause)` を受信した `fault_recreate` が deferred 経由で `finish_recreate` に cause を引き渡す
- **THEN** `pre_restart(&mut ctx, &cause)` と `post_restart(&mut ctx, &cause)` に渡される `ActorErrorReason` は `handle_failure` で構築されたものと `PartialEq` で一致する

#### Scenario: テストコードは `Recreate(ActorErrorReason::new(...))` で構築する

- **WHEN** 既存テスト (`actor_cell/tests.rs`, `system_message/tests.rs`, `system/base/tests.rs`) が `SystemMessage::Recreate` を構築する
- **THEN** 必ず `ActorErrorReason` 引数を渡す形式に更新されている

### Requirement: pre_restart / post_restart は cause 参照を受け取らなければならない

`Actor::pre_restart` / `Actor::post_restart` は `(&mut ActorContext, &ActorErrorReason)` のシグネチャを持ち、kernel は restart path で cause を渡さなければならない（MUST）。

#### Scenario: kernel は pre_restart に cause を渡す

- **WHEN** `fault_recreate(cause)` が `pre_restart` を呼び出す
- **THEN** 呼び出しは `actor.pre_restart(&mut ctx, &cause)` となり `cause` への参照が渡される

#### Scenario: kernel は post_restart に cause を渡す

- **WHEN** `finish_recreate(cause)` が `post_restart` を呼び出す
- **THEN** 呼び出しは `actor.post_restart(&mut ctx, &cause)` となり `cause` への参照が渡される

#### Scenario: default pre_restart は子全停止後 post_stop を呼ぶ

- **WHEN** `Actor::pre_restart` の default 実装が実行される
- **THEN** `ctx.stop_all_children()` が呼ばれる
- **AND** `self.post_stop(ctx)` が呼ばれる

#### Scenario: default post_restart は pre_start に委譲する

- **WHEN** `Actor::post_restart` の default 実装が実行される
- **THEN** `self.pre_start(ctx)` が呼ばれる

### Requirement: WatchKind は user watch と supervision watch を区別しなければならない

`ActorCellState` の `watchers` / `watching` フィールドは `Vec<(Pid, WatchKind)>` 型を持ち、`WatchKind { User, Supervision }` で watch の起源を区別しなければならない（MUST）。これは internal supervision watch が user の `unwatch` 操作によって誤って解除されないことを保証するための correctness 要件である。

#### Scenario: `watching` / `watchers` は kind 情報を保持する

- **WHEN** `ActorCellState` の型定義を確認する
- **THEN** `watchers: Vec<(Pid, WatchKind)>` と `watching: Vec<(Pid, WatchKind)>` が定義されている
- **AND** `WatchKind` は `User` と `Supervision` の 2 variant を持つ

#### Scenario: 同一 pid が User / Supervision 両方で登録されうる

- **GIVEN** 親 `P` が child `C` を `WatchKind::Supervision` で自動登録している
- **WHEN** `P` の実装が `ctx.watch(&C_ref)` を呼んで user-level watch を追加する
- **THEN** `P.state.watching` には `(C_pid, WatchKind::Supervision)` と `(C_pid, WatchKind::User)` の 2 エントリが保持される
- **AND** 同一 `(pid, kind)` の重複登録は冪等に扱われる（1 つのみ保持）

#### Scenario: `unwatch` は User 登録のみを除去し Supervision を保持する

- **GIVEN** 親 `P` が child `C` を両 kind で登録している状態（上記 Scenario の結果）
- **WHEN** `P` の実装が `ctx.unwatch(&C_ref)` を呼ぶ
- **THEN** `P.state.watching` から `(C_pid, WatchKind::User)` が除去される
- **AND** `(C_pid, WatchKind::Supervision)` は保持される
- **AND** `C` 側の `handle_unwatch(P_pid)` も同様に `(P_pid, WatchKind::User)` のみを除去する

#### Scenario: supervision watch は user unwatch 後も AC-H4 を駆動する

- **GIVEN** 親 `P` が child `C` に対して user watch を追加し、その後 `unwatch` を呼んだ状態
- **AND** `P` は `SystemMessage::Recreate(cause)` を受信し deferred 状態に入っている
- **WHEN** `C` を含む最後の child が停止し `DeathWatchNotification(C_pid)` が `P` に届く
- **THEN** `P.state.watching` には `(C_pid, WatchKind::Supervision)` が残っているため `watching_contains_pid(C_pid)` が真
- **AND** `handle_death_watch_notification` が駆動され `finish_recreate(cause)` が発火する

### Requirement: 親は子の spawn 時に internal supervision watch と children 登録を TOCTOU-safe に自動配線しなければならない

`spawn_with_parent` (`system/base.rs:605`) は以下の順序で internal supervision watch と children 登録を行わなければならない（MUST）:

1. `register_cell(cell)` で child cell を system に登録
2. **`register_child(parent_pid, pid)`** で parent の `children_state` に child を登録（Create handshake より前）
3. **`perform_create_handshake` より前に** 両サイドの `WatchKind::Supervision` 登録を行う（watch 両サイド登録）
4. `perform_create_handshake(parent, pid, &cell)` で `SystemMessage::Create` を送信

step 2 と step 3 を step 4 より前に行うことで、child が `pre_start` で即座に失敗して停止した場合でも:
- `notify_watchers_on_stop` が既に登録済みの parent へ `DeathWatchNotification(child_pid)` を送信できる
- parent の `handle_death_watch_notification` が `remove_child_and_get_state_change(pid)` を呼んだとき、child が `children_state` に登録済みのため `Some(state_change)` が返る（レースで `None` を返し `finish_recreate` が起動しない事態を回避）

#### Scenario: `spawn_with_parent` は handshake 前に children と両サイド watch を登録する

- **WHEN** `spawn_with_parent(Some(parent_pid), props)` が実行される
- **THEN** `perform_create_handshake` 呼び出しの直前に以下 3 件の登録が完了している:
  - parent の `children_state` に `pid` が追加されている
  - child cell の `state.watchers` に `(parent_pid, WatchKind::Supervision)` が追加されている
  - parent cell の `state.watching` に `(pid, WatchKind::Supervision)` が追加されている

#### Scenario: Create 直後に child が pre_start で失敗しても parent の `finish_recreate` が駆動される

- **GIVEN** `spawn_with_parent` が `register_cell` / `register_child` / internal watch 登録を完了し、`perform_create_handshake` を実行した状態
- **AND** parent が `SystemMessage::Recreate(cause)` を受信して deferred 状態にあり、child は restart 再生成途中
- **WHEN** child actor が `SystemMessage::Create` 処理中の `pre_start` で失敗し、即座に `handle_stop` 経由で停止する
- **THEN** `notify_watchers_on_stop` が `state.watchers` 内の `parent_pid` を参照でき、parent へ `SystemMessage::DeathWatchNotification(child_pid)` が送信される
- **AND** parent の `handle_death_watch_notification` が `remove_child_and_get_state_change(pid)` を呼び、child は `children_state` に既に登録済みのため `Some(SuspendReason::Recreation(cause))` が返る
- **AND** `finish_recreate(cause)` が発火する

#### Scenario: `rollback_spawn` は children 登録と internal watch を両方巻き戻す

- **WHEN** `perform_create_handshake` が失敗して `rollback_spawn` が呼ばれる
- **THEN** 以下の巻き戻しが行われる:
  - parent cell の `state.watching` から `(pid, WatchKind::Supervision)` が除去される
  - parent の `children_state` から child の pid が除去される（`unregister_child(parent_pid, pid)` 相当）
  - child cell は `remove_cell` で破棄される（watchers は一緒に消える）
- **AND** 巻き戻し後、parent 側に child 関連の state が残らない（`children_state.contains(pid)` / `state.watching_contains_pid(pid)` ともに偽）

#### Scenario: child 停止時に parent へ DeathWatchNotification が届く

- **GIVEN** `spawn_child` で生成された child cell（user は明示的に watch していない）
- **WHEN** child cell が停止し `handle_stop` 末尾の `notify_watchers_on_stop` が呼ばれる
- **THEN** parent へ `SystemMessage::DeathWatchNotification(child_pid)` が送信される
- **AND** parent の `handle_death_watch_notification` が呼ばれ、`remove_child_and_get_state_change(child_pid)` 経由で restart state machine が駆動される

#### Scenario: restart 中の child 全員停止で finish_recreate が発火する

- **GIVEN** 親が `SystemMessage::Recreate(cause)` を受信し deferred 状態に入った
- **AND** 親は internal supervision watch により child 群を全員 `state.watching` に `WatchKind::Supervision` で持っている
- **WHEN** 最後の child が停止し `DeathWatchNotification(child_pid)` が parent に届く
- **THEN** `handle_death_watch_notification` が `remove_child_and_get_state_change` で `Some(SuspendReason::Recreation(cause))` を得る
- **AND** `finish_recreate(cause)` が発火される

#### Scenario: `spawn_child_watched` は internal watch と冪等に併存する

- **WHEN** user が `ActorContext::spawn_child_watched(props)` を呼ぶ
- **THEN** internal supervision watch (spawn 時自動登録の `WatchKind::Supervision`) に加えて、`spawn_child_watched` 内の `watch(child.actor_ref())` が `WatchKind::User` で追加の watching 登録を行う
- **AND** 2 つの kind が並存する形で冪等に扱われる

### Requirement: Watch/Unwatch は watching/terminated_queued を同期更新しなければならない

`ActorContext::watch` / `unwatch` / `watch_with` は `ActorCellState` の `watching` / `terminated_queued` を次の不変条件に従って更新しなければならない（MUST）。

#### Scenario: watch は watching に `WatchKind::User` で追加する

- **WHEN** `ActorContext::watch(pid)` が呼ばれる
- **THEN** `state.watching` に `(pid, WatchKind::User)` が追加される（同一 `(pid, kind)` の重複登録は冪等）

#### Scenario: unwatch は User 登録と terminated_queued のみ除去する

- **WHEN** `ActorContext::unwatch(pid)` が呼ばれる
- **THEN** `state.watching` から `(pid, WatchKind::User)` のみが除去される
- **AND** `(pid, WatchKind::Supervision)` が登録されていれば保持される
- **AND** `state.terminated_queued` から `pid` が除去される（DeathWatchNotification 配送中の in-flight dedup もキャンセル）
- **AND** `state.watch_with_messages` から `pid` の entry が除去される

### Requirement: DeathWatchNotification 受信時は kernel が直接 on_terminated を呼び dedup を行わなければならない

被 watch 側 kernel から送信される `SystemMessage::DeathWatchNotification(Pid)` は、watcher kernel で `watching` 判定と `terminated_queued` dedup を行い、通過した場合に **kernel が直接 `actor.on_terminated(&mut ctx, pid)` を呼ばなければならない**（MUST、user mailbox への enqueue を経由しない）。`SystemMessage::DeathWatchNotification(Pid)` は kernel 内で Terminated を伝搬する**唯一の system queue envelope** とし、`SystemMessage::Terminated(Pid)` variant は本 change で enum 定義から**削除**される（MUST、「後方互換を保つコードを書かない」原則に従い未使用 variant を残さない）。

#### Scenario: watching 対象ならば kernel が直接 on_terminated を呼ぶ

- **WHEN** watcher の `system_invoke` が `SystemMessage::DeathWatchNotification(pid)` を受信する
- **AND** `state.watching_contains_pid(pid)` が真（kind 区別なく User / Supervision いずれかで登録）
- **AND** `state.terminated_queued.contains(&pid)` が偽
- **THEN** `state.watching` から `pid` の全 kind エントリが除去され、`state.terminated_queued` に `pid` が追加される
- **AND** `actor.on_terminated(&mut ctx, pid)` が kernel から直接呼ばれる
- **AND** user mailbox への enqueue は行われない

#### Scenario: `watch_with` 登録されていた場合は custom message が user queue に送られる

- **WHEN** watcher の `system_invoke` が `SystemMessage::DeathWatchNotification(pid)` を受信する
- **AND** `state.watch_with_messages` に `pid` のエントリがある
- **THEN** エントリの message が user mailbox に `actor_ref().try_tell(message)` として送信される
- **AND** この場合 `actor.on_terminated` は kernel から直接呼ばれない（custom message ハンドラが受信側で `on_terminated` 相当の処理を行う前提）

#### Scenario: 既に unwatch 済みなら silently drop

- **WHEN** watcher の `system_invoke` が `SystemMessage::DeathWatchNotification(pid)` を受信する
- **AND** `state.watching_contains_pid(pid)` が偽（User / Supervision どちらも登録されていない）
- **THEN** `on_terminated` は呼ばれない
- **AND** `state.terminated_queued` への追加も行われない

#### Scenario: 既に terminated_queued に居れば dedup される

- **WHEN** watcher の `system_invoke` が `SystemMessage::DeathWatchNotification(pid)` を受信する
- **AND** `state.terminated_queued.contains(&pid)` が真
- **THEN** 2 回目の `on_terminated` 呼び出しは行われない（dedup）

#### Scenario: `on_terminated` / `watch_with` 配送完了後に terminated_queued から除去される

- **GIVEN** `handle_death_watch_notification(pid)` の途中で `state.terminated_queued` に `pid` が追加された状態
- **WHEN** `actor.on_terminated(&mut ctx, pid)` または `watch_with` custom message の `try_tell` が完了する
- **THEN** その直後に同じ handler 内で `state.terminated_queued` から `pid` が除去される
- **AND** `terminated_queued` の保持期間は push から本 remove までの区間に限定される

#### Scenario: 既に停止済みの対象を watch した即時通知も同経路を通る

- **WHEN** `ActorCell::handle_watch(watcher)` が呼ばれ、対象 cell が既に `is_terminated()` である
- **THEN** watcher へ送信されるのは `SystemMessage::DeathWatchNotification(self.pid)` である（`SystemMessage::Terminated` を直送しない）
- **AND** watcher kernel 側で上記 `watching` 判定と `terminated_queued` dedup を経由して `on_terminated` が kernel から呼ばれる

#### Scenario: watch 送信先が closed の場合も即時通知は同経路を通る

- **WHEN** `ActorContext::watch(target)` が `send_system_message(target.pid(), SystemMessage::Watch(self.pid))` を呼び、`Err(SendError::Closed(_))` を受ける
- **THEN** 自己への補償通知は `SystemMessage::DeathWatchNotification(target.pid())` として送信される（`SystemMessage::Terminated` を自己送信しない）
- **AND** `state.watching` には事前に target が登録されており、統一経路の `watching` 判定・`terminated_queued` dedup を通過する

### Requirement: ChildrenContainer / SuspendReason の dead_code 許可は全解除されなければならない

restart completion の配線後、`modules/actor-core/src/core/kernel/actor/children_container.rs` および `suspend_reason.rs` の `#[allow(dead_code)]` アトリビュートは完全に除去されなければならない（MUST）。

#### Scenario: children_container の dead_code 許可が残っていない

- **WHEN** `modules/actor-core/src/core/kernel/actor/children_container.rs` を grep で検索する
- **THEN** `#[allow(dead_code)]` は 0 件である
- **AND** `shall_die` / `is_terminating` / `set_children_termination_reason` / `is_normal` は production コードから参照されている

#### Scenario: suspend_reason の dead_code 許可が残っていない

- **WHEN** `modules/actor-core/src/core/kernel/actor/suspend_reason.rs` を grep で検索する
- **THEN** `#[allow(dead_code)]` は 0 件である
- **AND** `SuspendReason::Recreation` variant は production コードから構築される

