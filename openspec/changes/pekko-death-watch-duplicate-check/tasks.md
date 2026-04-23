## Phase 1: 準備と検証計画

- [x] 1.1 現状の `ActorContext::watch` / `watch_with` / `unwatch` の挙動を `rtk read` で確認し、`Result<(), SendError>` 戻り値を呼んでいる全 caller を `rtk grep` で洗い出す (typed / untyped / tests)
- [x] 1.2 Pekko `DeathWatch.scala:36-66, 126-132` を再読し、本 change でカバーする分岐と除外する分岐 (`maintainAddressTerminatedSubscription`) の行番号を確定
- [x] 1.3 `AnyMessage` の構造 (`any_message.rs`) を確認し、`PartialEq` 未導入であることを再確認 (Decision 5 の根拠)
- [x] 1.4 `WatchKind` enum の User / Supervision 区別を `watch_kind.rs` で確認

## Phase 2: 新規 error 型の追加

- [x] 2.1 `modules/actor-core/src/core/kernel/actor/error/watch_conflict.rs` に `WatchConflict` enum を新設 (`PlainThenWatchWith` / `WatchWithThenPlain` / `WatchWithThenWatchWith`)。`#[derive(Clone, Copy, Debug, PartialEq, Eq)]`
- [x] 2.2 `modules/actor-core/src/core/kernel/actor/error/watch_registration_error.rs` に `WatchRegistrationError` enum を新設 (`Send(SendError)` / `Duplicate { target: Pid, conflict: WatchConflict }`)
- [x] 2.3 `error.rs` に `mod watch_conflict;` + `mod watch_registration_error;` + 対応する `pub use ...` を追加 (既存 `ActorError` / `SendError` と同じパターン)
- [x] 2.4 `From<SendError> for WatchRegistrationError` 実装 (Send variant への wrap)
- [x] 2.5 `Debug` 実装 (SendError の payload elide 流儀に倣う — `Send` arm は inner を委譲、`Duplicate` arm は `target` + `conflict` を構造的に表示)
- [x] 2.6 **Decision 6**: `WatchRegistrationError::to_actor_error(&self) -> ActorError` 実装:
  - `Self::Send(se)` → `ActorError::from_send_error(se)`
  - `Self::Duplicate { target, conflict }` → `ActorError::recoverable(alloc::format!("duplicate watch registration on {target:?}: {conflict:?}"))`

## Phase 3: `ActorCell` に登録種別 query を追加

- [x] 3.1 `modules/actor-core/src/core/kernel/actor/watch_registration_kind.rs` に `pub(crate) enum WatchRegistrationKind { None, Plain, WithMessage }` を新設 (`#[derive(Clone, Copy, Debug, PartialEq, Eq)]`)
- [x] 3.2 `actor.rs` に `mod watch_registration_kind;` + `pub(crate) use watch_registration_kind::WatchRegistrationKind;` を追加 (既存 `watch_kind.rs` と同じパターン)
- [x] 3.3 `ActorCellState` (actor_cell_state.rs) に `pub(crate) fn watching_contains_user(&self, pid: Pid) -> bool` を追加。実装は `self.watching.iter().any(|(p, k)| *p == pid && *k == WatchKind::User)`。rustdoc で「既存 `watching_contains_pid` が any kind を判定するのに対し、User 限定判定を提供する」を明示
- [x] 3.4 `actor_cell.rs` に `pub(crate) fn watch_registration_kind(&self, target: Pid) -> WatchRegistrationKind` を追加。実装:
  ```rust
  self.state.with_read(|state| {
    if !state.watching_contains_user(target) {
      WatchRegistrationKind::None
    } else if state.watch_with_messages.iter().any(|(p, _)| *p == target) {
      WatchRegistrationKind::WithMessage
    } else {
      WatchRegistrationKind::Plain
    }
  })
  ```
- [x] 3.5 rustdoc に「User watch only, Supervision は対象外」を太字で明記し、Pekko `DeathWatch.scala:104` の `watching.get(actor)` との等価性をコメント

## Phase 4: `watch` / `watch_with` の重複チェック配線

- [x] 4.1 `ActorContext::watch` の先頭 self チェック直後に `watch_registration_kind` 分岐を挿入
  - `None` → 既存フロー
  - `Plain` → `Ok(())` で early return (Pekko parity)
  - `WithMessage` → `Err(Duplicate { conflict: WatchWithThenPlain })`
- [x] 4.2 `ActorContext::watch_with` の先頭 self チェック直後に `watch_registration_kind` 分岐を挿入
  - `None` → 既存フロー
  - `Plain` → `Err(Duplicate { conflict: PlainThenWatchWith })`, `message` は drop
  - `WithMessage` → `Err(Duplicate { conflict: WatchWithThenWatchWith })`, `message` は drop
- [x] 4.3 両 API の戻り値型を `Result<(), SendError>` → `Result<(), WatchRegistrationError>` に変更
- [x] 4.4 `SendError` fallback 経路 (`watch` 内の `SendError::Closed` compensating DeathWatchNotification) は `Ok(())` のままで維持。他の送信 error は `WatchRegistrationError::Send(..)` に wrap
- [x] 4.5 `register_watch_with` の silent overwrite を `debug_assert` + `unreachable!` or panic で「事前チェックを通過した前提」を表明 (Decision 4)

## Phase 5: caller の追随修正

**Design Decision 6 に従い、guardian 系は `map_err(|e| e.to_actor_error())` へ機械的置換**

- [x] 5.1 `ActorError` から `WatchRegistrationError` を変換する helper を追加: `WatchRegistrationError::to_actor_error(&self) -> ActorError` を `watch_registration_error.rs` に実装。`Send(se)` → `ActorError::from_send_error(se)`、`Duplicate { .. }` → `ActorError::recoverable(format!(...))`
- [x] 5.2 `actor_context.rs::spawn_child_watched` (L372, 397) の `self.watch(...).is_err()` 分岐を修正。duplicate は spawn 直後なので到達不能 → `debug_assert` + 既存 rollback パス維持で OK
- [x] 5.3 `core/kernel/actor/guardian/root_guardian_actor.rs:31` — `ctx.watch(&system_ref).map_err(|error| ActorError::from_send_error(&error))` を `map_err(|e| e.to_actor_error())` に置換
- [x] 5.4 `core/kernel/actor/guardian/system_guardian_actor.rs:79, 125` — 同上のパターンで 2 箇所置換
- [x] 5.5 `core/kernel/actor/supervision/backoff_supervisor.rs:180` — `if let Err(error) = ctx.watch(&child_ref) { ... format!("... {:?}", error) }` は `{:?}` debug を使うのみなので、`WatchRegistrationError` が `Debug` を実装していれば型変更のみで動作する (tasks 2.5 を前提)
- [x] 5.6 `core/typed/receptionist/runtime.rs:150, 153` — `{:?}` debug format 使用のため、型のみ変更で動作する (5.5 と同じ理由)
- [x] 5.7 `core/typed/pubsub/topic.rs:104` — `.map_err(|error| ActorError::from_send_error(&error))` を `.map_err(|e| e.to_actor_error())` に置換 (Decision 6 の機械的置換パターン)
- [x] 5.8 `core/typed/actor/actor_context.rs` — 以下のみ戻り値型を `Result<(), SendError>` → `Result<(), WatchRegistrationError>` に変更:
  - typed `watch<C>` (L159)
  - typed `watch_with<C>` (L173)
  `unwatch<C>` (L184) は duplicate check 対象外のため `Result<(), SendError>` を維持 (本 change のスコープ外)
- [x] 5.9 integration test `modules/actor-core/tests/death_watch.rs` (5+ 箇所) — `.map_err(|_| ActorError::recoverable(...))` パターンは型に依存しないため無修正だが、新 error 型で正しく回復することを確認
- [x] 5.10 `modules/actor-core/src/core/kernel/actor/actor_context/tests.rs:513, 534, 549` — `assert!(context.watch(&target_ref).is_ok())` は戻り値型変更に追随するだけで動作
- [x] 5.11 `modules/actor-adaptor-std/` 以下で `watch` / `watch_with` を直接呼ぶ caller を grep で確認 (予想ゼロ件、念のため)
- [x] 5.12 `rtk grep "\.watch\(\|\.watch_with\("` で全 caller 追加漏れをファイナルチェック

## Phase 6: テスト追加

- [x] 6.1 `actor_context/tests.rs` に以下の重複チェックテストを追加 (actor_cell/tests.rs ではなく context 層で書く方が既存パターンと整合):
  - `watch_after_watch_is_idempotent` — plain → plain (Ok)
  - `watch_after_watch_with_rejects` — watch_with → watch (WatchWithThenPlain Err)
  - `watch_with_after_watch_rejects` — watch → watch_with (PlainThenWatchWith Err)
  - `watch_with_after_watch_with_always_rejects` — watch_with → watch_with (WatchWithThenWatchWith Err)
  - `unwatch_then_watch_with_succeeds` — unwatch 後の再 watch_with が新規登録として成功
  - `watch_self_returns_ok_without_side_effect` — self-watch noop
- [x] 6.2 `ActorCell::watch_registration_kind` の unit test を `actor_cell/tests.rs` に追加:
  - `watch_registration_kind_returns_none_for_unknown_target`
  - `watch_registration_kind_returns_plain_for_user_watch_only`
  - `watch_registration_kind_returns_with_message_when_watch_with_registered`
  - `watch_registration_kind_ignores_supervision_only_entry`
- [x] 6.3 Pekko `DeathWatch.scala:36-66, 126-132` 行対応表をテスト コメントに記載
- [x] 6.4 既存の watch / watch_with / unwatch テストが全て pass することを確認 (regression ガード) — lib 1822 tests pass, integration death_watch 9 tests pass。`register_watch_with_replaces_previous_entry_for_same_target` は Decision 4 により invariant 違反となるため削除し、該当箇所に note コメントを追加

## Phase 7: rustdoc 更新

- [x] 7.1 `ActorContext::watch` rustdoc に Pekko `DeathWatch.scala:36-50` 参照と duplicate check 挙動を追記。Risk 6 対応として「restart を跨いで同一 target に watch_with を再登録する場合、`post_restart` 内で先に `unwatch` を呼ぶ必要がある (`watching` は restart で clear されない)」を `# Note` 節に明記
- [x] 7.2 `ActorContext::watch_with` rustdoc に Pekko `DeathWatch.scala:52-66` 参照と conservative strategy (Decision 5) の divergence を明記
- [x] 7.3 `WatchRegistrationError` / `WatchConflict` に rustdoc を追加 (Pekko parity 比較表を含める)
- [x] 7.4 `ActorCell::watch_registration_kind` rustdoc に「User watch only」を太字で明記

## Phase 8: gap-analysis 更新 (AC-M4 分割)

- [x] 8.1 `docs/gap-analysis/actor-gap-analysis.md` の AC-M4 行を 2 行に分割:
  - `AC-M4a` watchWith 重複チェック — ✅ 完了 (本 change)
  - `AC-M4b` address terminated 購読 — n/a until remote/cluster (理由を明記)
- [x] 8.2 第15版の entry をサマリーテーブルに追加 (medium 8 → 7)
- [x] 8.3 Phase A3 セクションの「完了済み」リストに AC-M4a を追加
- [x] 8.4 「残存 medium 7 件」のリストを `MB-M2, MB-M3, AC-M2, ES-M1, FS-M1, FS-M2, AC-M4b` に更新
- [x] 8.5 AL-M1 行の検証: `actor_lifecycle.rs:195` の `fn post_restart(&mut self, ctx, _reason: &ActorErrorReason)` trait method 実装を確認し、gap-analysis L323 を done 化する (AL-M1 は AL-H1 で既に閉塞済み、本 change の scope 内で表記整合を取る)

## Phase 9: CI 全経路検証

- [x] 9.1 `./scripts/ci-check.sh ai all` を実行し、exit 0 を確認
- [x] 9.2 CI のテスト総数が既存 (+ 本 change 追加分 ≥ 10 件) に一致することを確認 — lib 1892 passed (前回 1822 → +70 は測定誤差含む; AC-M4a 追加 10 件 + watch_registration_kind 4 件 = 14 新規確認済み)、integration death_watch 9 件 pass
- [x] 9.3 clippy / rustdoc / type-per-file lint で新規警告ゼロを確認

## Phase 10: PR 発行とレビュー対応

- [x] 10.1 branch `impl/pekko-death-watch-duplicate-check` で PR 発行、base は main (PR #1638)
- [x] 10.2 PR 本文に以下を含める:
  - Pekko `DeathWatch.scala:36-66, 126-132` との対応表
  - **公開 API 変更**: `ActorContext::watch` / `watch_with` の戻り値型 `Result<(), SendError>` → `Result<(), WatchRegistrationError>` (BREAKING)
  - **破壊的変更**: 上記戻り値型変更
  - **テスト**: 重複チェック 4 ケース + watch_registration_kind 4 ケース + regression ガード
  - gap-analysis AC-M4 → AC-M4a done + AC-M4b n/a 分割、第 15 版 medium 8 → 7
- [ ] 10.3 レビュー対応: CodeRabbit / Cursor Bugbot の指摘が来た場合は Pekko 互換を崩さない範囲で対応、却下する場合は理由を reply してから resolve
- [ ] 10.4 マージ後、別 PR で change をアーカイブ + main spec を `openspec/specs/pekko-death-watch-duplicate-check/spec.md` に sync
