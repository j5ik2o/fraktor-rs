## Why

`MailboxOverflowStrategy::Block` を中心とする bounded mailbox の async backpressure 機能群は、半実装のまま design debt として残っている。

- **Production caller がゼロ**: `grep "MailboxOverflowStrategy::Block"` の結果、actor の `props.with_mailbox_overflow(Block)` のような production 利用箇所は存在しない。テストとインフラコードのみ
- **実装が常に broken だった**:
  - Phase 11.8 (`abbc26313` `feat(dispatcher): route MailboxOfferFuture through DispatcherWaker`) で `drive_offer_future` を導入したが busy-loop hack だった (InlineExecutor では同期 drain で偶然動作、非 inline executor では spin)
  - PR #1525 マージ後に Cursor Bugbot が High severity で `drive_offer_future busy-loop` を指摘 (`#3043806318`)
  - 修正コミット `6ce9b357 fix(dispatcher): address Bugbot review findings on PR #1525` で busy-loop を削除し、`MessageDispatcher::dispatch` default impl が `EnqueueOutcome::Pending(future)` を `SendError::full` で即返すように変更。これにより Block の "wait for capacity" semantics は完全に無効化された
- **`dispatcher_waker` モジュールが dead code 化**: PR #1525 で Bugbot から `dispatcher_waker is dead code` (`#3043448944` の波及) を指摘されたが、`drive_offer_future` 経由以外に呼び出し元がない
- **参照実装と整合しない**:
  - **Apache Pekko**: `BoundedMailbox` は JVM の `LinkedBlockingQueue` で thread parking する semantics を持つが、Pekko 自身が `Mailboxes.scala:259-263` で「`pushTimeOut > 0`（=ブロック設定）は問題があるので 0 に設定するか `NonBlockingBoundedMailbox` を使え」と警告を出している (非推奨)
  - **Proto.Actor Go**: `actor/bounded.go` は `Bounded(N)` (lossy log-and-drop) と `BoundedDropping(N)` (drop-oldest) の 2 種類のみで、thread parking 系は **存在しない**
- **fraktor-rs の no_std + multi-target 制約と相性が悪い**: 正しい sync block 実装には `std::thread::park` 相当の primitive を `fraktor-utils-rs` に新規追加する必要があり、実需要ゼロのために投資する価値がない
- **`DropNewest` / `DropOldest` / `Grow` の 3 戦略で実需要はカバー**: Proto.Actor Go の `BoundedDropping` 相当 (`DropOldest`)、Pekko の `NonBlockingBoundedMailbox` 相当 (`DropNewest` / `DropOldest`)、unbounded grow (`Grow`) の 3 つで Pekko 推奨パスと Proto.Actor Go の現実的な選択肢を両方カバー済み

`Less is more` / `YAGNI` / 「後方互換不要」のプロジェクト方針 (`CLAUDE.md`) に基づき、broken な半実装を撤去して codebase を正直な状態にする。

## What Changes

### 削除対象 (本体)

- `dispatcher_waker.rs` モジュール + 単体テスト (3 件) — Bugbot dead code 指摘の解消
- `MailboxOverflowStrategy::Block` enum variant
- `BoundedMessageQueue` の `Block` match arm (残り 3 戦略のみで構成)
- `BoundedPriorityMessageQueue` / `BoundedStablePriorityMessageQueue` の `Block` reject 分岐 (priority queue では元々サポートしていなかった)
- `MailboxOfferFuture` モジュール本体 + 単体テスト (4 件)
- `EnqueueOutcome::Pending` variant + `EnqueueOutcome` enum 自体 (Pending 撤去後は variant が `Enqueued` 1 つのみとなり enum の存在意義がなくなる)
- `MessageQueue::enqueue` trait method の戻り値を `Result<EnqueueOutcome, SendError>` から `Result<(), SendError>` に簡素化 (trait 変更につき以下すべての実装に波及)
- `DequeMessageQueue::enqueue_first` trait method も同様に簡素化
- `Mailbox::enqueue_envelope` / `Mailbox::enqueue_user` の戻り値を `Result<(), SendError>` に簡素化
- `Mailbox::prepend_user_messages` 内の `EnqueueOutcome` 分岐
- `MessageDispatcher::dispatch` default impl の `Pending(_future)` 分岐 (戻り値型変更により消滅)
- `Executor::supports_blocking()` trait method (default 含む)
- `ExecutorShared::supports_blocking()` convenience method
- `SpawnError::InvalidMailboxConfig` variant + `invalid_mailbox_config()` constructor (caller ゼロ)
- `mailbox_queue_handles.rs` の `MailboxOverflowStrategy::Block => OverflowPolicy::Block` mapping

### 削除対象 (queue 実装すべて — `MessageQueue::enqueue` trait 変更による波及)

- `bounded_message_queue.rs` + `bounded_message_queue/tests.rs`
- `bounded_priority_message_queue.rs` + `bounded_priority_message_queue/tests.rs`
- `bounded_stable_priority_message_queue.rs` + `bounded_stable_priority_message_queue/tests.rs`
- `unbounded_message_queue.rs` + `unbounded_message_queue/tests.rs`
- `unbounded_control_aware_message_queue.rs` + `unbounded_control_aware_message_queue/tests.rs`
- `unbounded_priority_message_queue.rs` + `unbounded_priority_message_queue/tests.rs`
- `unbounded_stable_priority_message_queue.rs` + `unbounded_stable_priority_message_queue/tests.rs`
- `unbounded_deque_message_queue.rs` + `unbounded_deque_message_queue/tests.rs`
- `dispatcher/shared_message_queue.rs` + `dispatcher/shared_message_queue/tests.rs`
- `dispatcher/balancing_dispatcher.rs` 内の `SharedMessageQueueBox` (`MessageQueue` impl)
- `mailbox/base.rs` 本体と `mailbox/base/tests.rs` の mock queue 実装

### 削除対象 (`supports_blocking` impl + テスト + rustdoc — `Executor` trait method 変更による波及)

- `inline_executor.rs` の `supports_blocking()` impl
- `inline_executor/tests.rs` の `supports_blocking_returns_false` テスト
- `executor_shared/tests.rs` の `supports_blocking_query` テスト + mock executor の `supports_blocking` impl
- `tokio_executor.rs` (std adapter) の `supports_blocking()` impl
- `tokio_executor/tests.rs` の `supports_blocking_returns_true` テスト
- `threaded_executor.rs` (std adapter) の `supports_blocking()` impl
- `threaded_executor/tests.rs` の `supports_blocking_returns_true` テスト
- `pinned_executor.rs` (std adapter) の `supports_blocking()` impl
- `pinned_executor/tests.rs` の `supports_blocking_returns_false` テスト
- `system/state/system_state/tests.rs` の mock executor の `supports_blocking` impl
- `system/base/tests.rs` の mock executor の `supports_blocking` impl
- `executor.rs` の rustdoc 内の `supports_blocking` 言及
- `actor_cell.rs::create` の rustdoc 内の "Returns SpawnError::InvalidMailboxConfig if ..." 説明 (現コードではこの分岐自体が存在しない死文)

### 触らない範囲 (現コードに存在しないもの)

- `MessageDispatcherShared::attach` 内の `supports_blocking()` 検証ゲート — **現コードには存在しない** (PR #1525 の `e442989c` 時点から実装されておらず、`message_dispatcher_shared.rs:98-106` の `attach` は `register_actor` + `register_for_execution` のみ実行)。spec delta としては archive 後 baseline から要件を MODIFIED するが、実装タスクとしては no-op

### 残る overflow 戦略

`MailboxOverflowStrategy` は以下の 3 戦略のみとなる:

| Variant | Semantics | 参照実装での対応 |
|---|---|---|
| `DropNewest` | full のとき新着を drop | Pekko `NonBlockingBoundedMailbox` 相当 |
| `DropOldest` | full のとき先頭を drop | Proto.Actor Go `BoundedDropping(N)` |
| `Grow` | unbounded grow | Pekko `UnboundedMailbox` / Proto.Actor Go `Unbounded()` |

### 触らない範囲 (non-goals)

- `fraktor-utils-rs::OverflowPolicy::Block` — stream module (`stream-core/src/core/impl/fusing/stream_buffer_config.rs:46`) が default として使っているため**削除しない**。actor mailbox の Block と stream の Block は **別レイヤー**
- `WaitShared` / `QueueState::register_producer_waiter` — stream module の async backpressure で稼働中のため**削除しない**
- `ExecutorShared` の trampoline と AShared 構造 — supports_blocking 削除以外は触らない

### Capabilities

#### Modified Capabilities

- `dispatch-executor-unification`:
  - REMOVED: `Requirement: \`DispatcherWaker\` は core 層に 1 実装で提供される`
  - MODIFIED: `Requirement: \`Executor\` trait は CQS 準拠の internal primitive として再定義される` (scenario 内の `supports_blocking` 言及を削除、`InlineExecutor::supports_blocking` シナリオの該当行も削除、`ExecutorShared` convenience method 列から `supports_blocking` を削除)
- `dispatcher-attach-detach-lifecycle`:
  - MODIFIED: `Requirement: dispatcher は 1 : N で actor を収容する lifecycle を提供する` (scenario `attach は mailbox overflow strategy と executor の blocking 対応を検証する` を削除)

## Sequencing

- **依存**: 本 change の `specs/` (`dispatch-executor-unification` / `dispatcher-attach-detach-lifecycle`) は **`dispatcher-pekko-1n-redesign` が archive された後の baseline** に対する MODIFIED / REMOVED Requirements を含む。`dispatcher-pekko-1n-redesign` archive 後に validate / apply する想定
- **現コードと spec の不一致を本 change が利用する**: `dispatcher-pekko-1n-redesign/specs/dispatcher-attach-detach-lifecycle/spec.md:21-25` は「`MessageDispatcherShared::attach` が `MailboxOverflowStrategy::Block` の場合に `supports_blocking()` を検証する」と要求しているが、**現コードの `message_dispatcher_shared.rs:98-106` には当該ゲートが存在しない**。本 change は当該 scenario を MODIFIED で削除することで、spec を実装に追従させる役割も兼ねる。実装側のタスクとしては「ゲートを削除する」ではなく「該当する rustdoc (`actor_cell.rs:157`) を削除する」のみで足りる
- **代替路**: 仮に `dispatcher-pekko-1n-redesign` の archive がさらに大幅に遅延する場合は、本 change の spec delta を `dispatcher-pekko-1n-redesign/specs/` の in-place 修正に切り替える代替路もある (proposal/tasks/design は変更不要)。判断は archive のタイミングを見て確定する

## Impact

### 影響コード (mailbox 層)

- `modules/actor-core/src/core/kernel/dispatch/mailbox/`
  - `overflow_strategy.rs` + `overflow_strategy/tests.rs` (`Block` variant 削除)
  - `mailbox_enqueue_outcome.rs` (enum ごと削除)
  - `mailbox_offer_future.rs` + `mailbox_offer_future/tests.rs` (削除)
  - `message_queue.rs` (`enqueue` の戻り値型変更)
  - `deque_message_queue.rs` (`enqueue_first` の戻り値型変更)
  - `bounded_message_queue.rs` + `bounded_message_queue/tests.rs` (Block arm + trait 戻り値型追従)
  - `bounded_priority_message_queue.rs` + `bounded_priority_message_queue/tests.rs` (Block reject + trait 戻り値型追従)
  - `bounded_stable_priority_message_queue.rs` + `bounded_stable_priority_message_queue/tests.rs` (Block reject + trait 戻り値型追従)
  - `unbounded_message_queue.rs` + `unbounded_message_queue/tests.rs` (trait 戻り値型追従)
  - `unbounded_control_aware_message_queue.rs` + `unbounded_control_aware_message_queue/tests.rs` (trait 戻り値型追従)
  - `unbounded_priority_message_queue.rs` + `unbounded_priority_message_queue/tests.rs` (trait 戻り値型追従)
  - `unbounded_stable_priority_message_queue.rs` + `unbounded_stable_priority_message_queue/tests.rs` (trait 戻り値型追従)
  - `unbounded_deque_message_queue.rs` + `unbounded_deque_message_queue/tests.rs` (trait 戻り値型追従)
  - `mailbox_queue_handles.rs` (`Block` mapping 削除)
  - `base.rs` + `base/tests.rs` (`enqueue_envelope` / `enqueue_user` / `prepend_user_messages` / mock queue 戻り値型追従、rustdoc 整理)
  - `mailbox.rs` (`pub use` 整理: `MailboxOfferFuture`, `EnqueueOutcome` の export 削除)

### 影響コード (dispatcher 層)

- `modules/actor-core/src/core/kernel/dispatch/dispatcher/`
  - `dispatcher_waker.rs` + `dispatcher_waker/tests.rs` (削除)
  - `dispatcher.rs` (`mod dispatcher_waker` / `pub use dispatcher_waker::dispatcher_waker` 削除)
  - `executor.rs` (trait method `supports_blocking` 削除 + rustdoc 整理)
  - `inline_executor.rs` (`supports_blocking` impl 削除)
  - `inline_executor/tests.rs` (`supports_blocking_returns_false` テスト削除)
  - `executor_shared.rs` (`supports_blocking` convenience method 削除)
  - `executor_shared/tests.rs` (`supports_blocking_query` テスト + mock impl 削除)
  - `shared_message_queue.rs` + `shared_message_queue/tests.rs` (trait 戻り値型追従)
  - `balancing_dispatcher.rs` (`SharedMessageQueueBox` の `MessageQueue` impl 戻り値型追従)
  - `message_dispatcher.rs` (default `dispatch` impl の `Pending` 分岐削除 + `envelope_for_error` clone 削除)

### 影響コード (actor 層)

- `modules/actor-core/src/core/kernel/actor/`
  - `spawn/spawn_error.rs` (`InvalidMailboxConfig` variant + `invalid_mailbox_config()` constructor 削除)
  - `actor_cell.rs` (rustdoc 整理: `Returns SpawnError::InvalidMailboxConfig if ...` 行削除)
  - `system/state/system_state/tests.rs` (mock executor の `supports_blocking` impl 削除)
  - `system/base/tests.rs` (mock executor の `supports_blocking` impl 削除)

### 影響コード (std adapter)

- `modules/actor-adaptor-std/src/std/dispatch/dispatcher/`
  - `tokio_executor.rs` (`supports_blocking` impl 削除)
  - `tokio_executor/tests.rs` (`supports_blocking_returns_true` テスト削除)
  - `threaded_executor.rs` (`supports_blocking` impl 削除)
  - `threaded_executor/tests.rs` (`supports_blocking_returns_true` テスト削除)
  - `pinned_executor.rs` (`supports_blocking` impl 削除)
  - `pinned_executor/tests.rs` (`supports_blocking_returns_false` テスト削除)

### 影響 API (BREAKING)

- `MessageQueue::enqueue` trait method: 戻り値が `Result<EnqueueOutcome, SendError>` → `Result<(), SendError>` (BREAKING、すべての実装者に波及)
- `DequeMessageQueue::enqueue_first` trait method: 同上
- `Mailbox::enqueue_envelope` / `Mailbox::enqueue_user`: 戻り値が `Result<EnqueueOutcome, SendError>` → `Result<(), SendError>` (BREAKING)
- `MailboxOverflowStrategy`: `Block` variant 削除 (BREAKING)
- `EnqueueOutcome` enum: 完全削除 (BREAKING、戻り値が `Result<(), SendError>` で置換)
- `MailboxOfferFuture`: 完全削除 (BREAKING、ただし production caller ゼロ)
- `dispatcher_waker` 関数 / `DispatcherWaker` 型: 完全削除 (BREAKING、ただし production caller ゼロ)
- `Executor::supports_blocking()`: 削除 (BREAKING for downstream `Executor` 実装者、ただし production 実装者は inline + std adapter のみ)
- `ExecutorShared::supports_blocking()`: 削除 (BREAKING)
- `SpawnError::InvalidMailboxConfig` variant + `invalid_mailbox_config()` constructor: 削除 (BREAKING、ただし caller ゼロ)
- `MessageDispatcher::dispatch` default impl: 戻り値の中身は変えないが内部の `Pending` 分岐が消える (`envelope.clone()` も消える)

### 非対象

- 別の sync blocking primitive の追加 (`fraktor-utils-rs` への `Blocker` port 追加など) — 本 change のスコープ外。将来必要になったら独立 change として提案
- async send variant の追加 (`try_tell_async` のような non-blocking offer + future) — 本 change のスコープ外
- stream module の backpressure 機構 — 完全に独立、無関係
