## プロジェクト原則（全 change 共通）

本 change は以下 4 原則に従って設計される:

1. **Pekko 互換仕様の実現 + Rust らしい設計**: Pekko の内部セマンティクス（state machine、message envelope、lifecycle hook の順序）を厳密に再現しつつ、所有権・型安全性・no_std 制約を活かした Rust イディオムに翻訳する
2. **手間が掛かっても本質的な設計を選ぶ**: 段階的妥協や部分実装で済ませず、correctness を守るのに必要な変更は本 change で完結させる（本 change では WatchKind 分離を Phase A3 送りから繰り上げた判断がこれに該当）
3. **フォールバックや後方互換性を保つコードを書かない**: 未使用 variant は削除、破壊的変更は抑制しない、暫定実装は残さない（本 change では `SystemMessage::Terminated` variant 削除、`SystemMessage::Recreate(ActorErrorReason)` payload 化の caller 全更新がこれに該当）
4. **no_std core + std adaptor 分離**: `modules/actor-core` は no_std 維持（`alloc::*` のみ、`std::*` 禁止）、std 固有機能は `modules/actor-adaptor-std` に隔離。本 change は kernel 層のみを触るため `cfg-std-forbid` lint 違反を起こさない

## Why

Pekko `FaultHandling.scala` / `Children.scala` / `DeathWatch.scala` / `Actor.scala` との内部セマンティクス parity を完成させるため、Phase A2+ の密結合 4 項目 (AC-H2 cleanup / AC-H4 / AC-H5 / AL-H1 kernel wiring) を単一 change として配線する。

これらは Pekko の単一 state machine (`fault_recreate → handle_death_watch_notification → finish_recreate`) で駆動されるため、分割すると中間状態で `SystemMessage::Recreate` payload 型が壊れたり deferred cause が消費されない状態になり得る。したがって 1 change で完結させる必要がある（`handle_death_watch_notification` は現行 `handle_terminated` のリネーム + DeathWatchNotification handler 統合後の名称）。

現状の kernel はビルドエラー状態。`SystemMessage::Recreate(ActorErrorReason)` への破壊的変更が 5 箇所（`actor_cell.rs:1165/1309`, `system_state.rs:991`, `system_state_shared.rs:775`, `actor_cell.rs:1055` の `pre_restart` 引数不足）の caller を更新せずに途中停止している。

このブランチには以下の scaffold が既に存在する（本 change はこれらを消費する production 配線に焦点を絞る）:

- `failed_info.rs` (`FailedInfo` enum: `NoFailedInfo` / `FailedRef(Pid)` / `FailedFatally`)
- `actor_cell_state.rs` の `failed` / `watching` / `terminated_queued` / `deferred_recreate_cause` フィールド
- `actor_cell.rs` の `is_failed` / `is_failed_fatally` / `perpetrator` / `set_failed(pid)` / `set_failed_fatally` / `clear_failed` helper
- `system_message.rs` の `Recreate(ActorErrorReason)` variant と `DeathWatchNotification(Pid)` variant
- `actor_lifecycle.rs` の `pre_restart(&mut self, ctx, &ActorErrorReason)` / `post_restart(&mut self, ctx, &ActorErrorReason)` default 実装（Pekko 互換: `pre_restart` default は `ctx.stop_all_children()` + `post_stop`、`post_restart` default は `pre_start`）
- typed 層 `BehaviorSignal::PostRestart` variant (`behavior_signal.rs:19`) および `behavior_runner.rs:186` の dispatch
- `TypedActorAdapter::post_restart(&mut ctx, _reason)` 実装 (`typed_actor_adapter.rs:254`、reason は現状 drop)
- `typed/message_and_signals/post_restart.rs` の `PostRestart` signal

## What Changes

- `handle_recreate` を 2 フェーズ化する
  - `fault_recreate(cause)`: `pre_restart(&mut ctx, &cause)` → `debug_assert!(mailbox.is_suspended())` → `set_children_termination_reason(SuspendReason::Recreation(cause))` が `true` ならそのまま return（子の終了を待つ deferred 状態）/ `false` なら `finish_recreate(cause)` へ即時フォールスルー
  - `finish_recreate(cause)`: `drop_*` 群 → `recreate_actor` → `mailbox.resume()` → `clear_failed()` → `post_restart(&mut ctx, &cause)` 呼び出し、失敗時 `set_failed_fatally()` + supervisor エスカレーション
- 現行 `handle_terminated(pid)` (`actor_cell.rs:995-1017`) を **`handle_death_watch_notification(pid)` にリネームし、`SystemMessage::DeathWatchNotification(Pid)` handler として統合する**
  - `remove_child_and_get_state_change` の戻り値を dispatch:
    - `Some(Recreation(cause)) => finish_recreate(cause)`（AC-H4 完了駆動）
    - `Termination` / `Creation` は `TODO(Phase A3)` マーク（本 change の対象は `Recreation` 完了駆動のみ）
  - `watching` 判定と `terminated_queued` dedup は handler 冒頭で実施（AC-H5）
  - system_invoke の `SystemMessage::Terminated(pid)` match arm は削除（新経路では kernel 内から `Terminated` は送信されない）
- **親子 internal supervision watch の自動配線**（AC-H4 の前提を成立させるため必須）
  - 現行 `spawn_with_parent` (`system/base.rs:605`) は `register_child` で親子関係を登録するだけで watch は貼らない。通常の `spawn_child` は watch 経路を持たず、`spawn_child_watched` (`actor_context.rs:345`) のみが明示的に `watch` を呼ぶ別 API になっている
  - このままでは **親が子の `DeathWatchNotification` を受け取れず、AC-H4 の `finish_recreate` が発火しない**（`SystemMessage::Terminated` を廃止する本 change では致命的）
  - 対応として、`spawn_with_parent` が以下の順序で internal watch と children 登録を張る:
    1. `register_cell(cell)` — child cell を system に登録
    2. **`register_child(parent_pid, pid)`** — parent の `children_state` に child を登録（Create handshake より前に移動、`remove_child_and_get_state_change(pid)` が `None` を返すレースを回避）
    3. **internal watch を両側登録**（Create handshake より前、TOCTOU を回避、WatchKind-aware helper 経由）:
       - **child cell 側**: `child_cell.state.register_watcher(parent_pid, WatchKind::Supervision)` — 子が stop する際 `notify_watchers_on_stop` で親へ `DeathWatchNotification(child_pid)` が届くようにする
       - **parent cell 側**: `parent_cell.state.register_watching(pid, WatchKind::Supervision)` — 親が `handle_death_watch_notification` 冒頭の判定で child を通すようにする
    4. `perform_create_handshake(parent, pid, &cell)` — `SystemMessage::Create` を送信（この時点で既に watch + children 登録済みなので、child が `pre_start` で失敗して停止しても stop 通知が parent に届き、`remove_child_and_get_state_change` は `Some(state_change)` を返す）
  - 上記順序を守ることで、**Create 直後に child が pre_start で失敗・停止しても、`notify_watchers_on_stop` が既に登録済みの parent へ `DeathWatchNotification` を送信でき、`handle_death_watch_notification` で `remove_child_and_get_state_change(pid)` が正しく state_change を返せる**（TOCTOU 回避）
  - `perform_create_handshake` が失敗して `rollback_spawn` が走る場合は、step 2 / step 3 で張った登録をすべて helper 経由で巻き戻す:
    - parent cell で `state.unregister_watching(pid, WatchKind::Supervision)` を呼ぶ
    - parent の `children_state` から child を除去（`unregister_child(parent_pid, pid)`）
    - child cell は `remove_cell` で破棄（child 側 `watchers` の supervision 登録は cell と一緒に消える）
- **WatchKind 種別の導入**（user watch と internal supervision watch の混線を防ぐため）
  - 現行 `ActorCellState::watchers: Vec<Pid>` / `watching: Vec<Pid>` は種別を区別しない
  - これに対し `watch` / `unwatch` API は user 側、`spawn_with_parent` は supervision 側で同じ set を共有すると、**user が `ctx.unwatch(child)` を呼んだときに supervision watch も解除されて AC-H4 が壊れる**（correctness 破綻）
  - 対応として `WatchKind { User, Supervision }` enum を導入し、`ActorCellState` のフィールドを `Vec<(Pid, WatchKind)>` に変更する:
    - `state.watchers: Vec<(Pid, WatchKind)>` — child stop 時の通知先、`notify_watchers_on_stop` は kind 区別なく全員に `DeathWatchNotification` を送信
    - `state.watching: Vec<(Pid, WatchKind)>` — `handle_death_watch_notification` の判定は kind を区別せず `contains_pid(pid)` で通過させる
  - API セマンティクス:
    - `spawn_with_parent` は `WatchKind::Supervision` で両サイド登録
    - `ActorContext::watch` / `watch_with` は `WatchKind::User` で登録
    - `ActorContext::unwatch(target)`: parent's `watching` から `(target, User)` のみ除去。`(target, Supervision)` は保持。target 側 `watchers` へ `SystemMessage::Unwatch` を送信し、target の `handle_unwatch` は `(watcher, User)` のみ除去、`(watcher, Supervision)` は保持
    - 同一 pid が User / Supervision 両方で登録されることは冪等に許容する（例: user が明示的に親から子を watch した場合）
- `DeathWatchNotification(Pid)` 経路を配線する（**全 Terminated 配送経路を統一する**、`on_terminated` は kernel が直接呼ぶ）
  - system 側到達時に `watching` 判定 + `terminated_queued` dedup を行い、**未 dedup のときのみ `terminated_queued.push(pid)` + `actor.on_terminated(&mut ctx, pid)` を kernel が直接呼ぶ**（user mailbox への enqueue 経由にしない — Run 3 plan B1-D 準拠）
  - `ActorContext::watch` / `watch_with` / `unwatch` で `watching` の登録/解除を配線
  - `unwatch` 時は `terminated_queued` からも除去
  - **既存の即時通知経路を `DeathWatchNotification` に統一する**（統一しないと dedup 契約が一部ケースで破綻する）:
    - `ActorContext::watch` (`actor_context.rs:305`) 内 `SendError::Closed(_)` 時の自己 `SystemMessage::Terminated(target)` 送信を `SystemMessage::DeathWatchNotification(target)` へ変更
    - `ActorCell::handle_watch` (`actor_cell.rs:527`) の `is_terminated()` 分岐で `SystemMessage::Terminated(self.pid)` を送信している箇所を `SystemMessage::DeathWatchNotification(self.pid)` に変更（watcher 側で watching 判定と dedup を行わせる）
  - 上記統一により、`SystemMessage::Terminated(Pid)` は **kernel 内から送信元が消える**。「後方互換を保つコードを書かない」原則に従い、本 change で **variant 自体を削除する**（remote / cluster 経路で将来必要になれば該当 change で再導入）。system_invoke の `Terminated(pid)` match arm も削除、現行 `handle_terminated(pid)` 関数のロジック（`remove_child_and_get_state_change` + `watch_with_message` tell + `on_terminated` 呼び出し）は `DeathWatchNotification(pid)` handler に統合される
- `SystemMessage::Recreate(ActorErrorReason)` payload 化の caller を全更新する (5 箇所)
  - `actor_cell.rs:1165` (`handle_failure` 内 Restart directive)
  - `actor_cell.rs:1309` (`system_invoke` match arm を `Recreate(cause)` に)
  - `system_state.rs:991`, `system_state_shared.rs:775` (supervisor 経由の再帰 Recreate 送信)
  - テスト側 `actor_cell/tests.rs`, `system_message/tests.rs`, `system/base/tests.rs` を cause 付きに更新
- AL-H1 kernel 配線
  - `actor.pre_restart(&mut ctx, &cause)` の引数を付与（現在は `pre_restart(&mut ctx)` 呼び出しでコンパイルエラー）
  - `finish_recreate` 内で `actor.post_restart(&mut ctx, &cause)` を呼び、成功時に `publish_lifecycle(LifecycleStage::Restarted)` を発行
  - typed 側 `TypedActorAdapter` が kernel の `Actor::post_restart` から `BehaviorSignal::PostRestart` を発火
- AC-H2 cleanup
  - `children_container.rs` / `suspend_reason.rs` の `#[allow(dead_code)]` を全解除（`shall_die` / `is_terminating` / `set_children_termination_reason` / `is_normal` / `SuspendReason::Recreation` variant 等が全て production 配線された後）

## Capabilities

### New Capabilities
- `pekko-restart-completion`: restart completion と terminatedQueued routing を Pekko の 2 フェーズ state machine と整合させる

### Modified Capabilities
- `actor-runtime-safety`: `SystemMessage::Recreate` が cause payload を運び、restart path が `post_restart(reason)` を呼ぶ契約に更新

## Impact

- 対象コード:
  - `modules/actor-core/src/core/kernel/actor/actor_cell.rs` (handle_recreate → fault_recreate/finish_recreate 分割、handle_terminated → handle_death_watch_notification リネーム + DeathWatchNotification handler 統合、handle_failure / system_invoke の Recreate / DeathWatchNotification arm 更新)
  - `modules/actor-core/src/core/kernel/actor/actor_cell_state.rs` (`watchers` / `watching` を `Vec<(Pid, WatchKind)>` 化、`register_watcher` / `register_watching` / `unregister_*` / `watching_contains_pid` helper 追加)
  - `modules/actor-core/src/core/kernel/actor/watch_kind.rs` (**新規**、`WatchKind { User, Supervision }` enum)
  - `modules/actor-core/src/core/kernel/actor/actor_context.rs` (watch / unwatch / watch_with を `WatchKind::User` 指定に、`SendError::Closed` 時の `SystemMessage::Terminated` → `DeathWatchNotification` 統一)
  - `modules/actor-core/src/core/kernel/system/base.rs` (`spawn_with_parent` の TOCTOU-safe 順序変更: `register_cell` → `register_child` → internal watch 両サイド登録 → `perform_create_handshake`、`rollback_spawn` の巻き戻し拡張、`unregister_child` 追加)
  - `modules/actor-core/src/core/kernel/system/state/system_state.rs`, `system_state_shared.rs` (Recreate 送信)
  - `modules/actor-core/src/core/kernel/actor/messaging/system_message.rs` (`SystemMessage::Terminated(Pid)` variant 削除)
  - `modules/actor-core/src/core/kernel/actor/children_container.rs`, `suspend_reason.rs` (`#[allow(dead_code)]` 全解除)
  - `modules/actor-core/src/core/typed/internal/typed_actor_adapter.rs` (post_restart → PostRestart signal)
  - テスト: `actor_cell/tests.rs`, `actor_context/tests.rs`, `system_message/tests.rs`, `system/base/tests.rs`, `typed/internal/behavior_runner/tests.rs`
- 影響内容:
  - `SystemMessage::Recreate` variant の破壊的変更（payload: `ActorErrorReason`）。プロジェクトは正式リリース前のため許容
  - `Actor::pre_restart` / `Actor::post_restart` の signature が `&mut ctx, &ActorErrorReason` に確定
  - `DeathWatchNotification(Pid)` の観測挙動が追加される (watch 関係の公開 API は変更なし)
  - `#[allow(dead_code)]` 全解除後は `ChildrenContainer` / `SuspendReason` API が production 経路から全件参照される
- 非目標:
  - `actor_cell.rs` (1419 行) のファイル分割リファクタ（別 change）
  - lifecycle hooks (`pre_start` / `post_stop` / `pre_restart` / `post_restart`) への panic guard（`pekko-panic-guard` change で扱うのは `receive` のみ）
  - `SupervisorStrategy::handle_child_terminated` hook（Phase A3）
  - Pekko `faultCreate()` の `actor == null` 分岐と `finishTerminate` deferred（本 change は Recreation 完了駆動のみ）
  - typed 側 `Behavior::pre_restart` / `post_restart` へ `&ActorErrorReason` 引数を足す（現状 `typed_actor.rs:99/111` は reason なし、`typed_actor_adapter` で `_reason` を drop）。Run 3 plan B4 のうち `BehaviorSignal::PostRestart` の kernel→typed 配送は既に scaffold 済みで本 change で passing まで持ち込むが、typed 層で reason を扱うかどうかは Phase A3 送り
  - `FailedInfo::FailedRef(Pid)` variant の `handle_failure` 経路での本番利用（Run 3 plan で YAGNI 判定済。scaffold は残すが `set_failed(pid)` 呼び出しは本 change で配線しない。Phase A3 で `handle_failure` が perpetrator を記録する際に検討）
  - `WatchKind` を enum 形で公開して user に露出させること（`WatchKind` は `ActorCellState` 内部の区別手段として使い、`ActorContext::watch` の public API シグネチャは現状どおり `(&ActorRef)` で据え置く）
  - remote / cluster 経路の `DeathWatchNotification` 転送（本 change はローカル kernel のみ対象）

## 依存関係

- **本 change はビルドエラー 5 箇所を解消するため、同ブランチの B2 (`pekko-eventstream-subchannel`) / B3 (`pekko-panic-guard`) より先にマージする**。B2 / B3 も既存テストが書かれており `cargo test --workspace` の pass には kernel の compile が必要。
- B2 / B3 は本 change 完了後、相互に並列で実装可能（モジュール境界で独立）。
