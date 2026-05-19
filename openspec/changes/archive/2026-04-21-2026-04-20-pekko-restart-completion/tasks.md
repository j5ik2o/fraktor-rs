## 0. 前提確認（既存 scaffold の利用）

- [x] 0.1 ワーキングツリーに未コミットで存在する下記 scaffold が残っていることを確認する
  - `modules/actor-core/src/core/kernel/actor/failed_info.rs` (`FailedInfo` enum: `NoFailedInfo` / `FailedRef(Pid)` / `FailedFatally`)
  - `modules/actor-core/src/core/kernel/actor/children_container.rs`, `children_container/tests.rs`
  - `modules/actor-core/src/core/kernel/actor/suspend_reason.rs`, `suspend_reason/tests.rs`
  - `modules/actor-core/src/core/typed/message_and_signals/post_restart.rs`
  - `actor_cell_state.rs` の `failed` / `watching` / `terminated_queued` / `deferred_recreate_cause` フィールド
  - `actor_cell.rs` の `is_failed` / `is_failed_fatally` / `perpetrator` / `set_failed(pid)` / `set_failed_fatally` / `clear_failed` helper（行 363-428 付近）
  - `system_message.rs` の `Recreate(ActorErrorReason)` と `DeathWatchNotification(Pid)` variant
  - `actor_lifecycle.rs` の `pre_restart(&mut self, ctx, &ActorErrorReason)` / `post_restart(&mut self, ctx, &ActorErrorReason)` default 実装
  - typed 層: `BehaviorSignal::PostRestart` variant (`behavior_signal.rs:19`) + `behavior_runner.rs:186` dispatch + `typed/message_and_signals/post_restart.rs`
- [x] 0.2 現在のビルドエラー 5 箇所を確認する（`actor_cell.rs:1055/1165/1309` / `system_state.rs:991` / `system_state_shared.rs:775`）
- [x] 0.3 `FailedInfo::FailedRef(Pid)` variant は scaffold に残すが、本 change では `handle_failure` 経路から `set_failed(pid)` を呼ぶ配線を新規に追加しない（Run 3 plan で YAGNI 判定、Phase A3 で検討）。既存 scaffold は `#[allow(dead_code)]` なしでも無警告にビルドされる前提を確認する

## 1. `SystemMessage::Recreate(ActorErrorReason)` caller の全更新

- [x] 1.1 `ActorError::to_reason(&self) -> ActorErrorReason` の存在を grep で確認する。無ければ `modules/actor-core/src/core/kernel/actor/error/` に追加する（`Recoverable(r) | Fatal(r) | Escalate(r) => r.clone()` の形）
- [x] 1.2 `actor_cell.rs:1165` の `handle_failure` 内 `SupervisorDirective::Restart` 分岐で `SystemMessage::Recreate(actor_error.to_reason())` に更新する
- [x] 1.3 `system_state.rs:991` / `system_state_shared.rs:775` の supervisor 経由 Recreate 送信も `handle_child_failure` に渡された `error: &ActorError` を利用して `SystemMessage::Recreate(error.to_reason())` に更新する
  - **仮値（`ActorErrorReason::new("restart")` 等）を使ってはならない**。これらの送信は `handle_child_failure(pid, error, now)` の直後にあり、`error` は scope 内で利用可能。実 cause を保持することで `pre_restart(&mut ctx, &cause)` / `post_restart(&mut ctx, &cause)` まで MUST 伝播する
- [x] 1.4 テストの caller を更新する
  - `actor_cell/tests.rs` の `Recreate` 呼び出しを `Recreate(ActorErrorReason::new("test"))` 等（テスト用の任意 cause）に変更
  - `messaging/system_message/tests.rs` の variant 比較を payload 付きに変更
  - `system/base/tests.rs` の `send_system_message(pid, SystemMessage::Recreate(...))` を cause 付きに変更
- [x] 1.5 この時点で `actor_cell.rs:1055` (`pre_restart` 引数不足) と `actor_cell.rs:1309` (`Recreate` match arm) は section 2 で同時に解消されるまでコンパイルエラーのまま残る。本 section は caller / テスト側の更新に限定
- [x] 1.6 section 1 の変更範囲に対し `./scripts/ci-check.sh ai dylint` が exit 0（section 2 解消までは cargo check が通らない前提のため、dylint は section 2 完了時に実施する代わりにこの項目をスキップして良い — 実際のゲートは 2.8 に統合）

## 2. AC-H4 + AL-H1 kernel 配線: `fault_recreate` / `finish_recreate` + `post_restart`

**注**: section 2 は単一の state machine 書き換えを担うため原子的に行う。`handle_recreate` を分割しつつ同時に `pre_restart` の reason 引数追加と `post_restart` 呼び出しを配線する。

- [x] 2.1 `handle_recreate` を `fault_recreate(cause: ActorErrorReason)` へリネームし、以下のロジックに書き換える:
  - `pre_restart(&mut ctx, &cause)` 呼び出し（既存の引数不足エラー `actor_cell.rs:1055` を同時に解消）
  - `debug_assert!(self.mailbox().is_suspended())` で AC-H3 前提を裏取り
  - `self.state.with_write(|s| s.deferred_recreate_cause = Some(cause.clone()))`
  - `ChildrenContainer::set_children_termination_reason(SuspendReason::Recreation(cause.clone()))` が `true` なら return（deferred）
  - `false`（子がいない）なら `self.finish_recreate(cause)` に即時フォールスルー
- [x] 2.2 `finish_recreate(cause: ActorErrorReason)` を新設する:
  - 冒頭で `self.state.with_write(|s| s.deferred_recreate_cause.take())` で deferred クリア
  - `drop_pipe_tasks` / `drop_stash_messages` / `drop_timer_handles` / `drop_watch_with_messages` 実行
  - `publish_lifecycle(LifecycleStage::Stopped)` 発行
  - `recreate_actor` 実行
  - `self.clear_failed()` 呼び出し
  - `self.mailbox().resume()` 呼び出し
  - `actor.post_restart(&mut ctx, &cause)` 呼び出し（AL-H1 配線の本体）、成功時 `publish_lifecycle(LifecycleStage::Restarted)`
  - 失敗時 `self.set_failed_fatally()` + `self.report_failure(&error, None)` でエスカレーション
- [x] 2.3 `system_invoke` の `SystemMessage::Recreate` match arm を `Recreate(cause) => cell.fault_recreate(cause)` に書き換える（`actor_cell.rs:1309` 解消）
- [x] 2.4 現行 `run_pre_start(Restarted)` 呼び出しは restart path から除去される（`post_restart` default が `pre_start` に委譲するため二重呼びにならない）
- [x] 2.5 typed 層: `TypedActorAdapter::post_restart(&mut ctx, _reason)` (`typed_actor_adapter.rs:254`) が既に scaffold 済みで `behavior_runner.rs:186` の `BehaviorSignal::PostRestart` dispatch を駆動することを確認する（production 変更不要、passing を確認するのみ）
- [x] 2.6 **子なし restart 経路の AC-H4 テスト** (`actor_cell/tests.rs` の `ac_h4_t1`: 子なしで `Recreate(cause)` → 同期 fallthrough) と **AL-H1 テスト** (`actor_cell/tests.rs` の `al_h1_*`、`behavior_runner/tests.rs` の `al_h1_*`) が passing することを確認する
  - **注**: 子あり restart テスト (`ac_h4_t2..t5`) は new section 4 の `handle_death_watch_notification` dispatch 完成まで pass しない。この時点では ignore / skip 状態で良い（section 5.8 で全 passing 化を再確認する）
- [x] 2.7 この時点で `rtk cargo check -p fraktor-actor-core-rs` がクリーンビルドされる
- [x] 2.8 `./scripts/ci-check.sh ai dylint` が exit 0（section 1+2 の変更範囲が lint 規約を満たすこと）

## 3. WatchKind 基盤の導入（`ActorCellState` を tuple 化）

**本 section は new section 4 / 5 の前提**。handler 実装（new section 4）と `watch` / `unwatch` / `spawn_with_parent` 配線（new section 5）が `WatchKind`-aware な型・helper を必要とするため、先に基盤を整備する。

- [x] 3.1 **`WatchKind` enum の導入** — user watch と supervision watch の混線を防ぐ
  - `modules/actor-core/src/core/kernel/actor/watch_kind.rs` を新規作成
    - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub(crate) enum WatchKind { User, Supervision }`
  - `modules/actor-core/src/core/kernel/actor.rs` に `pub(crate) mod watch_kind;` + `pub(crate) use watch_kind::WatchKind;` を追加
- [x] 3.2 `modules/actor-core/src/core/kernel/actor/actor_cell_state.rs` を改修
  - `watchers: Vec<Pid>` → `watchers: Vec<(Pid, WatchKind)>`
  - `watching: Vec<Pid>` → `watching: Vec<(Pid, WatchKind)>`
- [x] 3.3 `ActorCellState` に helper を追加
  - `fn watching_contains_pid(&self, pid: Pid) -> bool` — kind 区別なく任意の kind で登録があるか
  - `fn watchers_contains_pid(&self, pid: Pid) -> bool` — 同上（watcher 側）
  - `fn register_watching(&mut self, pid: Pid, kind: WatchKind)` — `watching` に冪等追加（同一 `(pid, kind)` は 1 つのみ）
  - `fn unregister_watching(&mut self, pid: Pid, kind: WatchKind)` — `watching` から該当 kind のみ除去
  - `fn register_watcher(&mut self, pid: Pid, kind: WatchKind)` — `watchers` 側の同等操作
  - `fn unregister_watcher(&mut self, pid: Pid, kind: WatchKind)` — 同上
- [x] 3.4 既存 `watching.push(pid)` / `watching.contains(&pid)` / `watchers.push(pid)` 等の直接操作を grep で洗い出し、すべて helper 呼び出しに置き換える
  - `actor_cell.rs` の `handle_watch` / `handle_unwatch` / `watchers_snapshot` 等
  - `actor_context.rs` の `watch` / `watch_with` / `unwatch`
  - 既存テストは push 形式を使う場合があるが、test helper も新 API に揃える
- [x] 3.5 `rtk cargo check -p fraktor-actor-core-rs` でクリーンビルドを確認
- [x] 3.6 `./scripts/ci-check.sh ai dylint` が exit 0

## 4. AC-H4 + AC-H5: `handle_terminated` を `DeathWatchNotification` handler に統合

（section 2 の `fault_recreate` / `finish_recreate` 完了後に実施）

現状の `handle_terminated(pid)` (`actor_cell.rs:995-1017`) は system queue 側で `remove_child_and_get_state_change` + `watch_with_message` tell + `actor.on_terminated` 呼び出しを担っている。本 section でこの全ロジックを新しい `DeathWatchNotification(pid)` handler に移行する。

- [x] 4.1 現 `handle_terminated(pid)` 関数を **`handle_death_watch_notification(pid)` にリネーム** する（rename-symbol）
- [x] 4.2 `handle_death_watch_notification` に以下を順序どおり実装する（section 3 で導入済みの `WatchKind`-aware helper を前提とする。`state.watching` / `state.watchers` は `Vec<(Pid, WatchKind)>` 型）:
  1. `state.watching_contains_pid(pid)` が偽なら silently return（User / Supervision どちらも未登録、既に unwatch 済みか初めから watch していない）
  2. `state.terminated_queued.contains(&pid)` が真なら silently return（dedup、同一 pid の重複 notification を防ぐ）
  3. **両 kind の entry を一括除去** + `state.terminated_queued.push(pid)` を atomic に行う:
     - `state.watching.retain(|(p, _kind)| *p != pid)` で `User` / `Supervision` 両方の entry を一度に除去する（watch 元が死んだ時点でどちらの kind も意味を失う）
     - `state.terminated_queued.push(pid)` で dedup 用 marker を設置
  4. `ChildrenContainer::remove_child_and_get_state_change(pid)` の戻り値を取得
  5. `take_watch_with_message(pid)` があれば `actor_ref().try_tell(message)` で user queue に送信、なければ `actor.on_terminated(&mut ctx, pid)` を kernel から直接呼ぶ（user mailbox enqueue 経由にしない）
  6. step 5 の送信・呼び出しが完了した直後に **`state.terminated_queued.retain(|p| *p != pid)` で `pid` を除去する**（dedup の保持期間は step 3 〜 step 6 の区間のみ、`on_terminated` / `try_tell` 完了後は解放して将来の new actor 同 pid 再利用に備える）
  7. step 4 の state_change が `Some(SuspendReason::Recreation(cause))` なら `self.finish_recreate(cause)` を呼ぶ（`cause` は state_change payload から取得、`deferred_recreate_cause` との一致を `debug_assert!` で裏取り）
  8. `Termination` / `Creation` は本 change では dispatch せず `// TODO(Phase A3): finish_terminate / finish_create` マークのみ残す
- [x] 4.3 `system_invoke` の `SystemMessage::DeathWatchNotification(pid) => cell.handle_death_watch_notification(pid)` arm を追加
- [x] 4.4 `system_invoke` の既存 `SystemMessage::Terminated(pid) => cell.handle_terminated(pid)` arm を削除する（kernel 内で `SystemMessage::Terminated` が送信されなくなるため）
- [x] 4.5 AC-H4 関連テスト（`actor_cell/tests.rs` の `ac_h4_t1..t5`）が passing することを確認する
- [x] 4.6 `./scripts/ci-check.sh ai dylint` が exit 0

## 5. AC-H5: `watching` / `terminated_queued` の登録・解除と通知経路統一

- [x] 5.1 **`ActorContext::watch` / `watch_with` を `WatchKind::User` 指定で登録する**
  - 登録は `state.register_watching(target.pid(), WatchKind::User)` helper 経由で行う（4.1 で追加）。直接 `watching.push` を呼ばない（重複時は冪等、同一 `(pid, kind)` は 1 つのみ保持）
  - `ActorContext::unwatch(target)` は `state.unregister_watching(target.pid(), WatchKind::User)` のみ呼び、`WatchKind::Supervision` 登録は保持する
  - target 側の `handle_unwatch(watcher)` も同様に `state.unregister_watcher(watcher, WatchKind::User)` のみ呼び、supervision 登録は維持する
- [x] 5.2 **親子 internal supervision watch + children 登録の自動配線（TOCTOU 回避順序）** — `system/base.rs:605` の `spawn_with_parent` を以下の順序に変更する（AC-H4 が `DeathWatchNotification` で駆動されるための前提）:
  1. `register_cell(cell.clone())` — 既存どおり
  2. **`register_child(parent_pid, pid)`** — parent の `children_state` に child を登録（既存は step 4 にあったが **Create handshake より前に移動**）
     - これにより `pre_start` 失敗後の `handle_death_watch_notification` が `remove_child_and_get_state_change(pid)` を呼んだ際に `Some(state_change)` が返る（レースで `None` を返す状態を回避）
  3. **internal watch を両サイド登録（Create handshake より前）**:
     - child cell 側: `cell.state.with_write(|state| state.register_watcher(parent_pid, WatchKind::Supervision))` — child stop 時 `notify_watchers_on_stop` で親へ通知
     - parent cell 側: `parent_cell.state.with_write(|state| state.register_watching(pid, WatchKind::Supervision))` — 親が `handle_death_watch_notification` で child を判定通過させる
  4. `perform_create_handshake(parent, pid, &cell)` — 既存どおり `SystemMessage::Create` 送信。ここで child が pre_start 失敗しても、watch + children 両方登録済みなので stop 通知が parent に届き、state_change が正しく返る
- [x] 5.3 **`perform_create_handshake` / `rollback_spawn` 失敗時の登録巻き戻し** — `rollback_spawn` (`system/base.rs:732`) 内で以下を追加（step 4.3 で Create より前に登録したすべての状態を巻き戻す）:
  - parent cell の `state.watching` から `(pid, WatchKind::Supervision)` を除去
  - parent の `children_state` から child を除去（`unregister_child(parent_pid, pid)` 相当、既存 API になければ追加）
  - child cell は `remove_cell` で破棄される（watchers は一緒に消えるため個別除去不要）
  - 巻き戻し順序は「watch 除去 → children 除去 → cell 除去」とし、部分失敗時も rollback 自体が冪等になるよう記述する
- [x] 5.4 `spawn_child_watched` (`actor_context.rs:345`) は user 明示 watch を追加する API として維持する — `watch` 呼び出しが `WatchKind::User` として重複登録を行い、既存 supervision watch と冪等共存する
- [x] 5.5 被 watch 側が stop する際、watcher 群へ `DeathWatchNotification(self.pid)` を送信する経路を `handle_stop` 末尾に追加する
  - 既存 `notify_watchers_on_stop` の送信先 envelope を `SystemMessage::Terminated(self.pid)` から `SystemMessage::DeathWatchNotification(self.pid)` に変更（関数名は維持可）
  - kind 区別は行わない — `User` / `Supervision` 両方の watcher 全員へ送信
- [x] 5.6 **即時通知経路の統一** — 既存コードで `SystemMessage::Terminated(pid)` を system queue に直送している 2 箇所を `SystemMessage::DeathWatchNotification(pid)` に統一する:
  - `actor_context.rs:305` `ActorContext::watch` 内 `SendError::Closed(_)` 分岐: `send_system_message(self.pid, SystemMessage::Terminated(target.pid()))` → `send_system_message(self.pid, SystemMessage::DeathWatchNotification(target.pid()))`
    - `5.1` で `state.watching` に target を `WatchKind::User` で事前登録しているため、統一経路の判定で正しく分岐される
  - `actor_cell.rs:527` `handle_watch` 内 `is_terminated()` 分岐: `send_system_message(watcher, SystemMessage::Terminated(self.pid))` → `send_system_message(watcher, SystemMessage::DeathWatchNotification(self.pid))`
    - 2 箇所（早期 return 側と通常通知側）両方を `DeathWatchNotification` に変更
- [x] 5.7 統一後、`SystemMessage::Terminated(Pid)` variant を **削除する**（kernel 内の送信元が存在しなくなるため、未使用コードを残さない「後方互換を保つコードを書かない」原則に従う）
  - `messaging/system_message.rs` の enum 定義から `Terminated(Pid)` variant を除去
  - `system_invoke` の match arm （section 4.4 で既に削除済み）と caller 群がすべて `DeathWatchNotification` に移行済みであることを grep で裏取り
  - remote / cluster 経路で将来必要になれば、その時点で該当 change で再導入する
- [x] 5.8 AC-H5 関連テスト（`actor_cell/tests.rs` の `ac_h5_t1..t6`）が全 passing することを確認し、以下の 4 観点を新規 integration test として追加する:
  - **既存 dedup 契約**: 「既に停止した actor を watch した場合」「watch 送信先 closed 時」の即時通知ケースが `terminated_queued` dedup を含めて passing することを確認（既存 `ac_h5_t1..t6` で検証）
  - **基本経路 (AC-H4)**: `spawn_child` で生成した child の停止が親の `handle_death_watch_notification` を駆動し、restart 中の親で `finish_recreate` が発火する（例: `ac_h4_parent_spawn_child_restart_completes_via_internal_watch`）
  - **TOCTOU — pre_start 失敗**: child が `pre_start` で即座にエラー停止しても、`children_state` と両サイド watch が Create より前に登録済みのため、parent が `DeathWatchNotification` を受信でき、`remove_child_and_get_state_change` が `Some` を返して `finish_recreate` が駆動する（例: `ac_h5_child_prestart_failure_notifies_parent_via_internal_watch`）
  - **TOCTOU — rollback_spawn**: `perform_create_handshake` が失敗したとき、parent の `children_state` / `state.watching` から child 関連の entry が完全に巻き戻される（例: `ac_h5_handshake_failure_rolls_back_children_and_watch`）
  - **WatchKind 分離**: parent が child を user-level `watch` / `unwatch` しても supervision watch は維持され、unwatch 後の child 停止で `finish_recreate` が依然として発火する（例: `ac_h5_user_unwatch_preserves_supervision_watch`）
- [x] 5.9 `./scripts/ci-check.sh ai dylint` が exit 0

## 6. AC-H2 cleanup

- [x] 6.1 `children_container.rs` の `#[allow(dead_code)]` を enum 本体から解除する
- [x] 6.2 `shall_die` / `is_terminating` / `set_children_termination_reason` / `is_normal` 各メソッドの `#[allow(dead_code)]` を解除する
- [x] 6.3 `suspend_reason.rs` の `#[allow(dead_code)]` 解除（`SuspendReason::Recreation` variant 含む全体）
- [x] 6.4 `FailedInfo::FailedRef(Pid)` variant は `#[allow(dead_code)]` を付与したまま残す（本 change で配線せず、Phase A3 の `handle_failure` 配線を待つ）
- [x] 6.5 `rtk cargo clippy -p fraktor-actor-core-rs -- -D dead_code` で残存 dead_code 警告がないことを確認
- [x] 6.6 `./scripts/ci-check.sh ai dylint` が exit 0（section 5 で `#[allow(dead_code)]` を大量に解除したことで lint 違反が出ていないこと）

## 7. 検証

- [x] 7.1 `rtk cargo test -p fraktor-actor-core-rs` 全テスト passing（AC-H4 / AC-H5 / AL-H1 スイート含む）
- [x] 7.2 section 2〜6 の各末尾で `./scripts/ci-check.sh ai dylint` を実行済みであることを再確認（本項目で追加実行する必要はない。`ai all` に dylint が含まれるため section 8.5 で最終実行される）

## 8. 品質ゲート（マージ前 MUST 条件）

本 change が proposal の 4 原則を満たしていることをマージ前に以下の項目で機械的に裏取りする。1 つでも fail したら該当作業に戻す。

### 8.1 原則 2 (本質的な設計を選ぶ) のゲート

- [x] 8.1.1 本 change の correctness に必要な変更がすべて完結していること
  - `rtk grep -rn "Phase A3" modules/actor-core/src/core/kernel/actor/` の結果が、proposal 「非目標」で明示済みの項目のみであること（新たに Phase A3 送りした correctness 関連 TODO がないこと）
- [x] 8.1.2 段階的妥協の痕跡がないこと（本 change で**新規追加**された TODO / workaround が許容範囲に限ること）
  - `git diff main...HEAD -- modules/actor-core/src/ | grep -E "^\+.*(TODO|FIXME|HACK|XXX|暫定|workaround|後で)" | grep -v "^\+.*TODO(Phase A3):"` が 0 行（`git diff` で変更行に限定することで、既存 TODO の誤検出と本 change での新規追加分を峻別する）
  - 許可される `TODO(Phase A3)` 項目は spec.md の「非目標」または proposal.md の「非目標」セクションに列挙済みのものに限る（`handle_death_watch_notification` 内の `Termination` / `Creation` dispatch 等）
- [x] 8.1.3 `WatchKind` 分離が完全実装されていること（Phase A3 送り案を繰り上げた判断の裏取り）
  - `rtk grep -rn "watching: Vec<Pid>\|watchers: Vec<Pid>" modules/actor-core/src/core/kernel/actor/` が 0 件
  - `state.watching.push(pid)` 形式の直接操作が 0 件（`register_watching` / `register_watcher` 経由）

### 8.2 原則 3 (後方互換性を保つコードを書かない) のゲート

- [x] 8.2.1 未使用 variant / 未使用 field / 未使用 method が 0 件
  - `rtk cargo clippy -p fraktor-actor-core-rs --all-targets -- -D dead_code` が exit 0
  - `#[allow(dead_code)]` は `FailedInfo::FailedRef` 1 箇所（Phase A3 で配線予定、spec 非目標で明示）のみ
- [x] 8.2.2 `SystemMessage::Terminated(Pid)` variant が enum 定義から除去されていること
  - `rtk grep -rn "SystemMessage::Terminated\b" modules/actor-core/src/` で caller / producer ともに 0 件
  - `messaging/system_message.rs` の enum 定義に `Terminated(Pid)` variant がないこと
- [x] 8.2.3 `SystemMessage::Recreate(ActorErrorReason)` の caller が全件 payload 付きに更新されていること
  - `rtk grep -rn "SystemMessage::Recreate\b[^(]" modules/actor-core/src/` で payload なし呼び出しが 0 件
  - 「暫定的に `ActorErrorReason::new("restart")`」等の固定値 cause が 0 件（`handle_child_failure` から伝播した実 cause のみ使用）
- [x] 8.2.4 `handle_terminated` → `handle_death_watch_notification` リネームが完了し、互換 alias / re-export が残っていないこと
  - `rtk grep -rn "fn handle_terminated\b" modules/actor-core/src/` が 0 件
- [x] 8.2.5 暫定 fallback / 後方互換 alias が 0 件
  - `rtk grep -rn "legacy\|compat\|deprecated\|backwards" modules/actor-core/src/core/kernel/actor/` で本 change 由来の互換コードがないこと

### 8.3 原則 4 (no_std core + std adaptor 分離) のゲート

- [x] 8.3.1 `rtk grep -rn "^use std::\|^use std$" modules/actor-core/src/core/kernel/actor/` が 0 件
- [x] 8.3.2 `cfg-std-forbid` dylint が違反を検出しないこと（下記 8.5.1 に含まれる）

### 8.4 Pekko 参照実装 parity のゲート

- [x] 8.4.1 本 change が proposal / design で参照している Pekko コード行（`FaultHandling.scala:92-118` / `:278-303` / `:327-351`, `Children.scala:178-188` / `:240-257`, `DeathWatch.scala`, `Actor.scala`）と実装が対応していること
- [x] 8.4.2 実装中に新たに発見した parity ギャップは proposal 「非目標」または spec の新 Requirement に追記済みであること（未記載のまま残さない）

### 8.5 CI / lint の final ゲート

- [x] 8.5.1 `./scripts/ci-check.sh ai all` が最終動作確認として exit 0（内部で dylint / cargo test / clippy / fmt を全件実行。8 custom lint 全 pass: mod-file / module-wiring / type-per-file / tests-location / use-placement / rustdoc / cfg-std-forbid / ambiguous-suffix。TAKT ルール上、このゲートは change のマージ直前にのみ実行）
