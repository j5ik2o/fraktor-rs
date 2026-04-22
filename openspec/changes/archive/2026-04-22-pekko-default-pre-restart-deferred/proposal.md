## Why

`2026-04-20-pekko-restart-completion` で restart completion の 2 フェーズ化は入ったが、**default `pre_restart` が `stop_all_children()` を呼ぶ経路**はまだ Pekko parity に達していない。

現状の fraktor-rs では同期 dispatcher / inline executor 環境で親が `SystemMessage::Recreate(cause)` を処理すると、

1. `fault_recreate` → `pre_restart` default → `ctx.stop_all_children()` が呼ばれる
2. `stop_all_children` は各 child を `mark_child_dying` で Terminating{UserRequest} へ遷移させ `SystemMessage::Stop` を送信する
3. `send_system_message(child, Stop)` は `MessageDispatcherShared::system_dispatch` → `register_for_execution` → `ExecutorShared::execute` を起動する
4. 親は mailbox 経由ではなく `ActorCellInvoker::system_invoke` で直接駆動されているため、**`ExecutorShared::running` フラグは false のまま**。したがって `ExecutorShared::execute` は `running.compare_exchange(false, true)` に成功し、drain owner になって child mailbox を drain する
5. child が停止すると `notify_watchers_on_stop` が親へ `DeathWatchNotification(child)` を送り、これが親 mailbox に enqueue されて再度 `ExecutorShared::execute(parent_run)` が呼ばれる。このとき `running=true` は既存 drain loop のため、parent_run は trampoline queue に積まれ、child_run 完了後の drain loop 次サイクルで同期実行される
6. 結果、parent の `handle_death_watch_notification` が fault_recreate スタック上で inline 実行される。この時点で `set_children_termination_reason(Recreation(cause))` はまだ呼ばれておらず、container は Terminating{UserRequest} のまま。`remove_child_and_get_state_change(child)` は `Some(SuspendReason::UserRequest)` を返し、`handle_death_watch_notification` は `finish_recreate` を駆動しない
7. stop_all_children が全 child 分ループを終えた時点で container は Normal/Empty、`pre_restart` 復帰後の `set_children_termination_reason(Recreation)` は `false` を返して `fault_recreate` は即時 `finish_recreate` に fall-through する

この挙動は Pekko の「default `pre_restart` でも子あり restart は deferred」という契約を破る。Pekko では dispatcher が child mailbox を別 turn で drain するため、`stop_all_children` 呼び出し中に container が空になることはない。

このズレは ignored テスト `al_h1_t2_default_pre_restart_stops_children_and_defers_finish_recreate` (`modules/actor-core/src/core/kernel/actor/actor_cell/tests.rs:1551`) が既に示している。問題は `stop_all_children` 単体ではなく、**`ActorCellInvoker::system_invoke` 直呼び経路が `ExecutorShared` の既存トランポリンを通らないため、親の処理スタック上で child / parent mailbox が drain されてしまう** という dispatch 境界の不足にある。

## What Changes

- **`ExecutorShared` 既存トランポリンの外部利用 API 追加**: `ExecutorShared` は既に `trampoline: SharedLock<TrampolineState>` + `running: ArcShared<AtomicBool>` を使った **外側トランポリン**を実装している（`executor_shared.rs:40-146`）。production 経路では `ExecutorShared::execute` が最初の呼び出しで `running=true` を CAS 確保し drain owner になり、再入呼び出しは pending queue に積むだけで drain しない。したがって production はこの既存機構のみで正しく deferred 挙動になる。本 change は、この既存機構に「外部から drain owner を宣言して execute を抑制する」API (`enter_drive_guard` / `exit_drive_guard`) を追加するだけで目的を達成する
- **ガード適用範囲は `fault_recreate` 内の `pre_restart` 呼び出し 1 点に限定**: `ActorCellInvoker::system_invoke` 全体や `fault_recreate` 全体、`finish_recreate` / `post_restart` には **ガードを適用しない**。これは default `pre_restart` のみが `stop_all_children` を呼ぶ唯一の Pekko 互換 lifecycle hook であり、他の system message 経路（`handle_watch` / `handle_unwatch` / `handle_stop` / `handle_kill` / `handle_failure` 等）には同様の再入問題が存在しないこと、およびガード範囲を狭く保つことで **既存の passing テストに新たな Pekko 非互換を生まない** ことを保証するため
- **`MessageDispatcherShared::run_with_drive_guard<F, R>(&self, f: F) -> R` 追加**: `f()` の実行前に `self.executor().enter_drive_guard()` を呼び、終了時に RAII ベースで `exit_drive_guard` を呼ぶ helper。panic 時も `exit_drive_guard` が呼ばれることを保証する
- **`Executor` trait は変更しない** (重要な設計修正): 5 ラウンド目レビューで判明した `ExecutorShared` 既存トランポリンの発見により、`Executor` trait に `enter_drive_guard` / `exit_drive_guard` を追加する当初案は **却下**。`InlineExecutor` への override も不要。guard は `ExecutorShared` レベルで完結する
- **ignored テスト `al_h1_t2_default_pre_restart_stops_children_and_defers_finish_recreate` の受け入れ条件化**: `#[ignore]` を外し、既定 `pre_restart` + `stop_all_children` + 明示的 `handle_death_watch_notification` → `finish_recreate` の flow が sync dispatch でも Pekko parity で pass することを CI ゲートで確認する。併せて test の `register_watching` (User) を `register_supervision_watching` (Supervision) に修正する（default pre_restart の `stop_all_children` が User watch を除去するため）
- **複数 child の deferred 挙動検証**: 現行 T2 は child 1 件のみを検証しているため、child 2 件を持つケースで「最後の child の DWN でのみ `finish_recreate` が起動する」順序性を追加 integration test で確認する
- **non-target の明示**: 本 change は restart 経路の deferred 契約のみを扱う。Termination（`finish_terminate`）経路の同様 deferred 化は Phase A3 送りのまま。typed 層の reason 伝播拡張、remote / cluster の死亡通知転送は扱わない。`stop_all_children` の API / 責務分割は本 change では変更しない

## Capabilities

### Modified Capabilities
- `pekko-restart-completion`: default `pre_restart` を含む全 restart 経路で、同期 dispatcher でも Pekko と同じ deferred completion 契約を満たすようにする
- `actor-runtime-safety`: `ExecutorShared` に `enter_drive_guard` / `exit_drive_guard` API を追加し、既存トランポリンの drain owner を外部から宣言できる契約を明文化する。`fault_recreate` の `pre_restart` 呼び出しは `MessageDispatcherShared::run_with_drive_guard` でラップされる

## Impact

- 対象コード（すべて `modules/actor-core` に閉じる見込み）:
  - `core/kernel/dispatch/dispatcher/executor_shared.rs` — `enter_drive_guard` / `exit_drive_guard` 追加。既存 `running: ArcShared<AtomicBool>` フィールドを操作
  - `core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs` — `run_with_drive_guard<F, R>(&self, f: F) -> R` を追加
  - `core/kernel/dispatch/dispatcher/message_dispatcher_shared/drive_guard.rs` (**新規ファイル**) — RAII `DriveGuard<'a>` struct を分離配置（親ファイルが既に 326 行で type-organization の同居行数制限を満たせないため）
  - `core/kernel/actor/actor_cell.rs` — `fault_recreate` 内の `actor.pre_restart(&mut ctx, cause)` 呼び出し 1 点を `run_with_drive_guard` でラップ（`ActorCellInvoker::system_invoke` 本体や `finish_recreate` / 他 handler には**触れない**）
  - `core/kernel/actor/actor_cell/tests.rs` — `al_h1_t2_*` の `#[ignore]` 削除、`register_watching` → `register_supervision_watching` 修正、複数 child 検証の integration test 追加
- Public API 変更:
  - `ExecutorShared` に新 API メソッドが 2 つ追加される (既存 `pub fn` 群と同じスタイル)
  - `Executor` trait は **変更なし**
- 影響範囲の明示:
  - production dispatcher (`PinnedExecutor` / `ForkJoinExecutor` / `BalancingDispatcher`) は ExecutorShared 既存トランポリンをそのまま使い続けるため挙動変化なし
  - `enter_drive_guard` を呼ばないコード経路 (= `fault_recreate` の `pre_restart` ラップ以外のすべて) は既存挙動のまま
  - 既存 passing テストのうち、`fault_recreate` を駆動しないものは guard の影響を一切受けない
  - `fault_recreate` を駆動する既存テスト（AC-H4 T1 / T2 / T3 / AL-H1 T1 / T3）は override pre_restart または子なしケースのため、guard 適用後も挙動不変
- 本 change は restart completion の correctness を確保する最終パッチ。`2026-04-21-2026-04-20-pekko-restart-completion` の parity claim を sync dispatcher でも成立させる

## Non-goals

- `stop_all_children` の「mark / unwatch / queue stop」責務分離（`ExecutorShared` 既存トランポリン + `enter_drive_guard` API で足りるため本 change では行わない。不足が観測されたら follow-up）
- panic guard や lifecycle hook 全般の例外処理（`pekko-panic-guard` change の守備範囲）
- remote / cluster の death watch 転送
- typed 側 `Behavior::pre_restart` / `post_restart` への reason 引数追加（Phase A3）
- `finish_terminate` / `finish_create` の deferred 化（Phase A3、本 change は Recreation 経路のみ）
- mailbox / dispatcher の一般的な性能最適化
- InlineExecutor を production dispatcher で使用可能にすること（test-only restriction は維持）
- `ActorCellInvoker::system_invoke` 全体や `fault_recreate` 全体、`finish_recreate` / `post_restart` への guard 適用（本 change では明示的に却下。理由は design.md 「却下案 1」参照）
- `Executor` trait の拡張（`ExecutorShared` レベルで完結するため不要）

## Dependencies

- **前提**: `2026-04-21-2026-04-20-pekko-restart-completion`（archive 済み）
- 本 change はその follow-up であり、既存 restart completion change の設計意図を保ったまま、同期 dispatcher で破れている deferred completion 契約を `ExecutorShared` 既存トランポリンの外部駆動 API で補完する
