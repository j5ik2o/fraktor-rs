## Why

Phase II (`mailbox-close-reject-enqueue`) で close correctness は `user_queue_lock` によって成立した。しかしこれは、`detach` 側が close と cleanup を直接所有し、`run()` 側が dequeue / invoke を所有するという **ownership の分裂** を lock で補っている形である。

Phase III / 3.5 により stash / prepend contract は deque-only に硬化され、generic fallback は production path から外れた。これにより次の論点は明確になった:

- mailbox user lane の最終責任者を誰にするか
- `cleanup vs dequeue` 競合を lock ではなく state machine で解消できるか

本 change は outer lock 全撤去を直接目的にしない。まず **cleanup ownership を `detach` から mailbox finalizer へ移す**。これにより `cleanup vs dequeue` 競合を構造的に解消し、後続 change で outer lock 削減を安全に再提案できる状態を作る。

## What Changes

- `MessageDispatcherShared::detach` は mailbox を「close requested」状態へ遷移させる orchestration に変更する
- 実際の queue drain / dead-letter / `clean_up()` は mailbox finalizer が **exactly once** で実行する
- finalizer は次のどちらかになる
  - mailbox が idle なら detach caller
  - mailbox が running なら in-flight runner
- `run()` は close request 観測後、追加の user dequeue を継続せず finalization へ移る
- `MailboxScheduleState` は「新規 schedule 拒否」と「cleanup 完了」を区別できる状態表現へ拡張する

## Non-Goals

- `user_queue_lock` の全面撤去
- `enqueue_envelope` / `prepend_user_messages_deque` / `user_len` / metrics publish の lock 配置最適化
- `MessageQueue` trait への queue-level close 導入
- `BalancingDispatcher` shared queue の close semantics 再設計
- prepend batch atomicity の queue primitive 化

### Follow-up Boundary

この change 完了後も、`user_queue_lock` に残る論点は次のとおりである。

- producer close race の authoritative re-check
- prepend batch atomicity
- metrics / `user_len` snapshot の一貫性

これらは後続の outer lock reduction change で扱う。

## Capabilities

### Modified Capabilities

- `mailbox-runnable-drain`
  - `run()` が terminal cleanup ownership を持てるようにする
- `dispatcher-attach-detach-lifecycle`
  - `detach` が direct cleanup ではなく close request + finalizer handoff を行うようにする
- `mailbox-close-semantics`
  - close correctness の authoritative owner を `user_queue_lock` 前提の cleanup から finalizer ownership へ更新する

## Impact

### 影響コード

- `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs`
- `modules/actor-core/src/core/kernel/dispatch/mailbox/schedule_state.rs`
- `modules/actor-core/src/core/kernel/dispatch/dispatcher/message_dispatcher_shared.rs`
- `modules/actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs`
- `modules/actor-core/src/core/kernel/actor/actor_cell/tests.rs` の mailbox detach / unstash ordering 周辺

### 設計上の効果

- `cleanup vs dequeue` 競合の責務境界が明確になる
- detach path が in-flight runner と競合して queue を直接 drain しなくなる
- outer lock reduction を次段に分離できる
