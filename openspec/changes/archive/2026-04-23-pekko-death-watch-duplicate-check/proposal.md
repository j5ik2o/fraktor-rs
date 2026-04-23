## Why

Pekko `dungeon/DeathWatch.scala:36-66` の `watch` / `watchWith` は、同一 target への二重登録を `checkWatchingSame` (`L126-132`) で検証し、**既存登録の message と新規 message が異なる場合は `IllegalStateException` を投げる**。この振る舞いは「`unwatch` せずに `watchWith(ref, M1)` → `watchWith(ref, M2)` で message が silently overwritten される事故」を実行時に検出する防衛線になっている。

fraktor-rs `actor_context.rs` の `watch` / `watch_with` にはこの重複チェックが無く、第2版以降の `register_watch_with` は既存 entry を無条件 retain で上書きする (`actor_cell.rs:1147`)。結果、ユーザーは watch_with 設定の衝突に気付けず、Terminated 配送時に「期待と異なる message が来る」silent bug が生じうる。

gap-analysis AC-M4 は 2 責務を束ねていた (重複チェック + `maintainAddressTerminatedSubscription`)。後者は remote/cluster の EventStream 基盤が整備されていない現状では実装不可のため、本 change では **重複チェック部分のみ** を完了させ、address-terminated 部分は gap-analysis で `n/a until remote/cluster complete` として切り離す。

## What Changes

- **BREAKING**: `ActorContext::watch` / `watch_with` の戻り値型を `Result<(), SendError>` から新規 `Result<(), WatchRegistrationError>` に変更する。typed `TypedActorContext::watch` / `watch_with` も同様に変更 (戻り値型連動)。`unwatch` は duplicate check 対象外のため `Result<(), SendError>` を維持。
- `ActorContext::watch` / `watch_with` が、同一 target に対して既に watching している状態で呼ばれた場合の重複検出を追加する。具体的には:
  - 既に plain `watch(target)` 済の相手に `watch_with(target, m)` — **Err(`WatchConflict::PlainThenWatchWith`)**
  - 既に `watch_with(target, m_prev)` 済の相手に `watch(target)` — **Err(`WatchConflict::WatchWithThenPlain`)**
  - 既に `watch_with(target, m_prev)` 済の相手に `watch_with(target, m_new)` — **Err(`WatchConflict::WatchWithThenWatchWith`)** (message 同一性の判定は `AnyMessage` に `PartialEq` が無いため実装不能、Pekko の conservative 上位集合として常に拒否する — design Decision 5)
  - 既に plain `watch(target)` 済の相手に `watch(target)` — **no-op で `Ok(())`** (Pekko parity: `None == None`)
- 新規公開型: `WatchRegistrationError` (`Send(SendError)` / `Duplicate { target, conflict }`) + `WatchConflict` enum を `kernel::actor::error` に追加。
- `WatchRegistrationError::to_actor_error()` helper を提供し、既存 caller の `ActorError::from_send_error(&e)` パターンを `e.to_actor_error()` へ機械的置換できる変換経路を用意する (design Decision 6)。
- 新規 pub(crate) 型: `WatchRegistrationKind` + `ActorCell::watch_registration_kind(pid)` query を追加。Supervision kind の watching entry と User kind を区別するため `ActorCellState::watching_contains_user(pid)` を追加。
- `actor_cell.rs::register_watch_with` の silent overwrite 挙動を廃止。上位で事前チェックするため、register 時点での衝突は `debug_assert!` で不変条件違反を表明する。
- gap-analysis AC-M4 行を **`AC-M4a` 「重複チェック」done** + **`AC-M4b` 「address terminated 購読」n/a (remote 依存)** に分割する。
- `maintainAddressTerminatedSubscription` 相当の implementation は本 change では扱わない。新しいギャップ `AC-M4b` として記録し、remote/cluster 基盤完成後に別 change で対応する。

## Capabilities

### New Capabilities
- `pekko-death-watch-duplicate-check`: DeathWatch における `watch` / `watchWith` の二重登録検出契約。同一 target への異種 watch 登録を `Err` として拒否し、silent overwrite を防ぐ。

### Modified Capabilities
<!-- 該当なし: DeathWatch 関連の既存 spec は存在しない。本 change で新規 capability として確立する。 -->

## Impact

**影響を受けるコード**:
- `modules/actor-core/src/core/kernel/actor/actor_context.rs::watch` / `watch_with` — 重複チェック分岐を追加
- `modules/actor-core/src/core/kernel/actor/actor_cell.rs::register_watch_with` — silent overwrite の廃止、衝突時はエラー
- `modules/actor-core/src/core/kernel/actor/actor_cell.rs` — 新関数 `check_watch_registration` or `watch_with_message` query accessor を追加
- 新規 error 種別: `WatchError` (新設) または `SendError::DuplicateWatch` variant 追加 (design で選択)

**影響を受ける API 契約**:
- untyped `ActorContext::watch` / `watch_with` の戻り値型: `Result<(), SendError>` → `Result<(), WatchRegistrationError>` (BREAKING)
- typed `TypedActorContext::watch` / `watch_with` の戻り値型: 同上 (BREAKING)
- `ActorContext::unwatch` / typed `unwatch` は影響なし (`Result<(), SendError>` 維持)

**影響を受けないもの**:
- `SystemMessage::Watch` / `Unwatch` の wire format
- `DeathWatchNotification` 配送経路
- `watched_by` / `watching` の data structure (watch_with_messages と合わせて参照するのみ)
- `terminatedQueued` dedup marker (`ActorCellState::terminated_queued`) は AC-H5 で既に実装済、本 change では触らない

**テスト**:
- `actor_cell/tests.rs` に重複チェック parity シナリオを追加 (7 ケース):
  1. plain → plain (Ok / Pekko `None == None` 相当)
  2. watch_with → plain (Err `WatchWithThenPlain`)
  3. plain → watch_with (Err `PlainThenWatchWith`)
  4. watch_with → watch_with (Err `WatchWithThenWatchWith` — conservative)
  5. unwatch → watch_with (Ok / 再登録)
  6. self-watch (Ok noop)
  7. 未登録 → watch_with (Ok / 新規登録 + rollback on send failure)
- `ActorCell::watch_registration_kind` unit tests (4 ケース): None / Plain / WithMessage / supervision-only 除外
- 既存 `watch` / `watch_with` テストは plain の初回登録が壊れないことの regression として維持

**gap-analysis 更新**:
- AC-M4 を `AC-M4a` (重複チェック — done) + `AC-M4b` (address terminated 購読 — n/a until remote) に分割
- 第15版として記録
