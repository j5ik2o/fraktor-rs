## 1. dispatcher_waker 撤去 (commit 1)

- [x] 1.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_waker.rs` を削除する
- [x] 1.2 `modules/actor-core/src/core/kernel/dispatch/dispatcher/dispatcher_waker/tests.rs` とディレクトリを削除する
- [x] 1.3 `modules/actor-core/src/core/kernel/dispatch/dispatcher.rs` から `mod dispatcher_waker;` と `pub use dispatcher_waker::dispatcher_waker;` を削除する
- [x] 1.4 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` の `enqueue_envelope` rustdoc から `DispatcherWaker` への言及を削除する (該当箇所: 旧 line 280-287 周辺)
- [x] 1.5 `cargo check -p fraktor-actor-core-rs --lib --tests` がコンパイル成功することを確認する
- [x] 1.6 `cargo test -p fraktor-actor-core-rs --lib core::kernel::dispatch::dispatcher` が pass することを確認する (59 passed)
- [x] 1.7 commit: `chore(mailbox): delete dead dispatcher_waker module` (e1c6a66b)

## 2. MailboxOverflowStrategy::Block variant 撤去 (commit 2)

`MailboxOfferFuture::new(...)` の唯一の production caller は `bounded_message_queue::enqueue` の `Block` arm。先にこれを消すことで commit 3 で `MailboxOfferFuture` を削除する際のコンパイルエラーを防ぐ。

### 2.A enum + bounded queue

- [x] 2.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/overflow_strategy.rs` から `Block` variant を削除する
- [x] 2.2 `modules/actor-core/src/core/kernel/dispatch/mailbox/overflow_strategy/tests.rs` の `Block` 関連 test を削除する
- [x] 2.3 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_message_queue.rs` の `MailboxOverflowStrategy::Block` match arm を削除する (`MailboxOfferFuture::new(...)` の production caller がここで消える)
- [x] 2.4 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue.rs` の `MailboxOverflowStrategy::Block` reject 分岐を削除する
- [x] 2.5 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_message_queue.rs` の `MailboxOverflowStrategy::Block` reject 分岐を削除する

### 2.B テスト追従

- [x] 2.6 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_message_queue/tests.rs` の Block 関連テストを削除する (Block 参照なし、no-op)
- [x] 2.7 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue/tests.rs` の Block reject test を削除する
- [x] 2.8 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_message_queue/tests.rs` の Block reject test を削除する
- [x] 2.9 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_mailbox_type/tests.rs` の Block 関連 test を確認・削除する (Block 参照なし、no-op)
- [x] 2.10 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_mailbox_type/tests.rs` の Block 関連 test を確認・削除する (Block 参照なし、no-op)

### 2.C 周辺整理

- [x] 2.11 `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_queue_handles.rs` の `MailboxOverflowStrategy::Block => OverflowPolicy::Block` mapping を削除する
- [x] 2.12 `MailboxPolicy::bounded(...)` を `MailboxOverflowStrategy::Block` で呼んでいる残テストを `DropNewest` 等に置換、または該当テストを削除する: `mailbox_offer_future/tests.rs` (4 件すべて Block 依存) を全削除し、`mailbox_offer_future.rs` から `mod tests;` を削除
- [x] 2.13 `cargo check -p fraktor-actor-core-rs --lib --tests` がコンパイル成功することを確認する (warnings 2 件 — `MailboxOfferFuture::new` dead code、commit 3 で解消)
- [x] 2.14 `cargo test -p fraktor-actor-core-rs --lib core::kernel::dispatch::mailbox` が pass することを確認する (117 passed)
- [x] 2.15 commit: `feat(mailbox): remove MailboxOverflowStrategy::Block variant` (dbf379b9)

## 3. MessageQueue::enqueue 戻り値簡素化 + MailboxOfferFuture / EnqueueOutcome 撤去 (commit 3)

`MessageQueue::enqueue` trait の戻り値型を `Result<EnqueueOutcome, SendError>` から `Result<(), SendError>` に変更し、`EnqueueOutcome` enum と `MailboxOfferFuture` を撤去する。trait 変更につき全 queue 実装 (~10 ファイル) + テストに波及する commit (本 PR で最大)。

### 3.A trait + 共通型

- [x] 3.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/message_queue.rs` の `MessageQueue::enqueue` 戻り値を `Result<(), SendError>` に変更する
- [x] 3.2 `modules/actor-core/src/core/kernel/dispatch/mailbox/deque_message_queue.rs` の `DequeMessageQueue::enqueue_first` 戻り値を `Result<(), SendError>` に変更する
- [x] 3.3 `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_enqueue_outcome.rs` ファイルごと削除 (`EnqueueOutcome` enum 撤去)
- [x] 3.4 `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_offer_future.rs` ファイルごと削除
- [x] 3.5 `modules/actor-core/src/core/kernel/dispatch/mailbox/mailbox_offer_future/tests.rs` とディレクトリを削除 (commit 2 で実施済み)
- [x] 3.6 `modules/actor-core/src/core/kernel/dispatch/mailbox.rs` (module declarations) から以下を削除:
  - `mod mailbox_enqueue_outcome;`
  - `mod mailbox_offer_future;`
  - `pub use mailbox_enqueue_outcome::EnqueueOutcome;`
  - `pub use mailbox_offer_future::MailboxOfferFuture;`

### 3.B queue 実装の追従

- [x] 3.7 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_message_queue.rs` を新シグネチャに追従 (`Ok(EnqueueOutcome::Enqueued)` → `Ok(())`)
- [x] 3.8 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue.rs` を新シグネチャに追従
- [x] 3.9 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_message_queue.rs` を新シグネチャに追従
- [x] 3.10 `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_message_queue.rs` を新シグネチャに追従
- [x] 3.11 `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_control_aware_message_queue.rs` を新シグネチャに追従
- [x] 3.12 `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_priority_message_queue.rs` を新シグネチャに追従
- [x] 3.13 `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_stable_priority_message_queue.rs` を新シグネチャに追従
- [x] 3.14 `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_deque_message_queue.rs` を新シグネチャに追従 (`enqueue` + `enqueue_first` 両方)
- [x] 3.15 `modules/actor-core/src/core/kernel/dispatch/dispatcher/shared_message_queue.rs` を新シグネチャに追従
- [x] 3.16 `modules/actor-core/src/core/kernel/dispatch/dispatcher/balancing_dispatcher.rs` 内の `SharedMessageQueueBox` の `MessageQueue` impl を新シグネチャに追従

### 3.C queue 実装テストの追従

- [x] 3.17 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_message_queue/tests.rs` の `Ok(EnqueueOutcome::Enqueued)` 等の match arm を `Ok(())` に置換 (no-op、参照なし)
- [x] 3.18 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_priority_message_queue/tests.rs` を追従 (no-op)
- [x] 3.19 `modules/actor-core/src/core/kernel/dispatch/mailbox/bounded_stable_priority_message_queue/tests.rs` を追従 (no-op)
- [x] 3.20 `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_message_queue/tests.rs` を追従 (no-op)
- [x] 3.21 `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_control_aware_message_queue/tests.rs` を追従 (no-op)
- [x] 3.22 `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_priority_message_queue/tests.rs` を追従 (no-op)
- [x] 3.23 `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_stable_priority_message_queue/tests.rs` を追従 (no-op)
- [x] 3.24 `modules/actor-core/src/core/kernel/dispatch/mailbox/unbounded_deque_message_queue/tests.rs` を追従 (no-op)
- [x] 3.25 `modules/actor-core/src/core/kernel/dispatch/dispatcher/shared_message_queue/tests.rs` を追従

### 3.D Mailbox 上位 API の追従

- [x] 3.26 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs::enqueue_envelope` の戻り値を `Result<EnqueueOutcome, SendError>` から `Result<(), SendError>` に変更
- [x] 3.27 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs::enqueue_user` を新しい戻り値型に追従
- [x] 3.28 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs::prepend_user_messages` 内の `Ok(EnqueueOutcome::Pending(_))` 分岐を削除し、`Ok(())` 分岐に簡素化 (`enqueue_for_prepend` ヘルパ自体を削除)
- [x] 3.29 `modules/actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs` の mock queue impl を新 trait シグネチャに追従
- [x] 3.30 `modules/actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs` 内の `Ok(EnqueueOutcome::Enqueued)` / `Ok(EnqueueOutcome::Pending(...))` を参照する test を削除または簡素化 (`mailbox_prepend_user_messages_blocks_pending_offer_poll_until_prepend_finishes` を削除、`ScriptedEnqueue::Pending` も削除)

### 3.E dispatcher hook の追従

- [x] 3.31 `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher.rs::dispatch` default impl を簡素化
- [x] 3.32 `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher.rs` の rustdoc から `Pending` / `MailboxOfferFuture` への言及を削除

### 3.F 検証

- [x] 3.33 `cargo check -p fraktor-actor-core-rs --lib --tests` がコンパイル成功することを確認する (warnings 2 件: `register_producer_waiter`, `len_handle` — backpressure infra 残骸、別 follow-up)
- [x] 3.34 `cargo check -p fraktor-actor-adaptor-rs --lib --tests` がコンパイル成功することを確認する
- [x] 3.35 `cargo test -p fraktor-actor-core-rs --lib` 全件 pass を確認する (1577 passed)
- [x] 3.36 `cargo test -p fraktor-actor-adaptor-rs --lib` 全件 pass を確認する (11 passed)
- [x] 3.37 `grep -rn "EnqueueOutcome\|MailboxOfferFuture" modules/` がヒット 0 を返すことを確認する
- [x] 3.38 commit: `refactor(mailbox): simplify MessageQueue::enqueue to Result<(), SendError> and drop MailboxOfferFuture` (bf29d745)

## 4. Executor::supports_blocking + SpawnError::InvalidMailboxConfig 撤去 (commit 4)

### 4.A trait + impl

- [x] 4.1 `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor.rs` から `supports_blocking()` trait method を削除する
- [x] 4.2 `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor.rs` の rustdoc から `supports_blocking` 言及を削除する
- [x] 4.3 `modules/actor-core/src/core/kernel/dispatch/dispatcher/inline_executor.rs` の `supports_blocking()` impl を削除する
- [x] 4.4 `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared.rs` の `supports_blocking()` convenience method を削除する

### 4.B std adapter impl

- [x] 4.5 `modules/actor-adaptor-std/src/std/dispatch/dispatcher/tokio_executor.rs` の `supports_blocking()` impl を削除する
- [x] 4.6 `modules/actor-adaptor-std/src/std/dispatch/dispatcher/threaded_executor.rs` の `supports_blocking()` impl を削除する
- [x] 4.7 `modules/actor-adaptor-std/src/std/dispatch/dispatcher/pinned_executor.rs` の `supports_blocking()` impl を削除する

### 4.C テスト + mock

- [x] 4.8 `modules/actor-core/src/core/kernel/dispatch/dispatcher/inline_executor/tests.rs` の `supports_blocking_returns_false` テストと `shutdown_clears_pending_tasks` 内の `assert!(!executor.supports_blocking())` 行を削除する
- [x] 4.9 `modules/actor-core/src/core/kernel/dispatch/dispatcher/executor_shared/tests.rs` の `supports_blocking_query` テスト + mock executor の `supports_blocking` impl + `blocking` フィールドを削除する
- [x] 4.10 `modules/actor-adaptor-std/src/std/dispatch/dispatcher/tokio_executor/tests.rs` の `supports_blocking_returns_true` テストを削除する
- [x] 4.11 `modules/actor-adaptor-std/src/std/dispatch/dispatcher/threaded_executor/tests.rs` の `supports_blocking_returns_true` テストを削除する
- [x] 4.12 `modules/actor-adaptor-std/src/std/dispatch/dispatcher/pinned_executor/tests.rs` の `supports_blocking_returns_false` テストを削除する
- [x] 4.13 `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs` の mock executor の `supports_blocking` impl 行を削除する
- [x] 4.14 `modules/actor-core/src/core/kernel/system/base/tests.rs` の mock executor の `supports_blocking` impl 行を削除する

### 4.D SpawnError::InvalidMailboxConfig 完全削除

- [x] 4.15 `modules/actor-core/src/core/kernel/actor/spawn/spawn_error.rs` の `SpawnError::InvalidMailboxConfig(String)` variant を削除する
- [x] 4.16 `modules/actor-core/src/core/kernel/actor/spawn/spawn_error.rs` の `invalid_mailbox_config()` constructor を削除する
- [x] 4.17 `modules/actor-core/src/core/kernel/actor/actor_cell.rs::create` の rustdoc 内 `Returns SpawnError::InvalidMailboxConfig if ...` 行を削除する
- [x] 4.18 `grep -rn "InvalidMailboxConfig\|invalid_mailbox_config" modules/` がヒット 0 を返すことを確認する

### 4.E 検証

- [x] 4.19 `cargo check -p fraktor-actor-core-rs --lib --tests` がコンパイル成功することを確認する
- [x] 4.20 `cargo check -p fraktor-actor-adaptor-rs --lib --tests` がコンパイル成功することを確認する
- [x] 4.21 `cargo test -p fraktor-actor-core-rs --lib` 全件 pass を確認する (1575 passed)
- [x] 4.22 `cargo test -p fraktor-actor-adaptor-rs --lib` 全件 pass を確認する (9 passed)
- [x] 4.23 `grep -rn "supports_blocking" modules/` がヒット 0 を返すことを確認する
- [x] 4.24 commit: `refactor(executor): remove supports_blocking trait method and SpawnError::InvalidMailboxConfig` (fa7a5fb3)

## 5. openspec spec delta 確認 + dead code 整理 (commit 5)

spec delta ファイルは proposal 作成時に既に作成済み・push 済み (4d8501a2)。本セクションでは検証と、commit 3 で残した backpressure infra の dead code 整理を実施する。

- [x] 5.1 `openspec/changes/mailbox-block-overflow-removal/specs/dispatch-executor-unification/spec.md` の内容を確認する:
  - REMOVED: `Requirement: \`DispatcherWaker\` は core 層に 1 実装で提供される`
  - MODIFIED: `Requirement: \`Executor\` trait は CQS 準拠の internal primitive として再定義される` (各 scenario 内の `supports_blocking` 関連行を削除済み)
- [x] 5.2 `openspec/changes/mailbox-block-overflow-removal/specs/dispatcher-attach-detach-lifecycle/spec.md` の内容を確認する:
  - MODIFIED: `Requirement: dispatcher は 1 : N で actor を収容する lifecycle を提供する` (scenario `attach は mailbox overflow strategy と executor の blocking 対応を検証する` を削除済み)
- [x] 5.3 `openspec validate mailbox-block-overflow-removal --strict` が valid を返すことを確認する
- [x] 5.4 dylint dead-code lint で検出される backpressure infra 残骸を削除する:
  - `mailbox/mailbox_queue_state.rs::register_producer_waiter` (caller ゼロ)
  - `mailbox/system_queue.rs::len_handle` (caller ゼロ)
- [ ] 5.5 commit: `chore(mailbox): clean up dead backpressure infrastructure and finalize tasks` (tasks.md の checkbox 確定 + dead code 削除を 1 コミットにまとめる)

## 6. 最終検証

- [ ] 6.1 `grep -rn "MailboxOverflowStrategy::Block" modules/ showcases/` がヒット 0 を返す
- [ ] 6.2 `grep -rn "MailboxOfferFuture" modules/ showcases/` がヒット 0 を返す
- [ ] 6.3 `grep -rn "dispatcher_waker\|DispatcherWaker" modules/ showcases/` がヒット 0 を返す (`openspec/` 配下を除く)
- [ ] 6.4 `grep -rn "EnqueueOutcome" modules/ showcases/` がヒット 0 を返す
- [ ] 6.5 `grep -rn "supports_blocking" modules/ showcases/` がヒット 0 を返す
- [ ] 6.6 `grep -rn "InvalidMailboxConfig\|invalid_mailbox_config" modules/ showcases/` がヒット 0 を返す
- [ ] 6.7 `cargo test -p fraktor-actor-core-rs --lib` 全件 pass
- [ ] 6.8 `cargo test -p fraktor-actor-adaptor-rs --lib` 全件 pass
- [ ] 6.9 `./scripts/ci-check.sh ai dylint` exit 0
- [ ] 6.10 `./scripts/ci-check.sh ai all` exit 0
- [ ] 6.11 `openspec validate mailbox-block-overflow-removal --strict` valid

## 7. PR 作成

- [ ] 7.1 PR title: `refactor(mailbox): remove MailboxOverflowStrategy::Block and async backpressure scaffolding`
- [ ] 7.2 PR description に proposal.md / design.md の要約と Pekko / Proto.Actor Go の比較根拠を含める
- [ ] 7.3 commit history が 5 つ (本体 4 つ + openspec 1 つ) に分かれていることを確認する
- [ ] 7.4 各コミットが独立して `cargo test` / `cargo check` を通過することを確認する
