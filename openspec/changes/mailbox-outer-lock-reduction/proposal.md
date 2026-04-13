## Why

`docs/plan/lock-strategy-analysis.md` の調査 A で判明した通り、Mailbox の `user_queue_lock` は **全ての enqueue/dequeue で取得されている** が、本来は compound op（prepend、close+cleanup）との排他のためだけに存在する。

現状のロック段数:
- `BoundedMessageQueue` 経路: **3 段** (user_queue_lock → QueueStateHandle → SyncQueueShared)
- `UnboundedDequeMessageQueue` 経路: **2 段** (user_queue_lock → inner mutex)

参照実装（Pekko / protoactor-go）は通常 enqueue で **0 段**（lock-free queue ベース）。

通常の enqueue/dequeue から `user_queue_lock` を外すことで:
- Bounded: 3 → 2 段
- Unbounded: 2 → 1 段

に削減できる。

## 前提条件（全て充足済み）

| 前提 | change | 状態 |
|---|---|---|
| close 時の is_closed 再チェック | `mailbox-close-reject-enqueue` | archive 済み |
| stash が deque mailbox を要求 | `stash-requires-deque-mailbox` | archive 済み |
| prepend が deque-only 契約 | `mailbox-prepend-requires-deque` | archive 済み |
| `prepend_via_drain_and_requeue` 削除 | 同上 | archive 済み |

## What Changes

- `Mailbox::enqueue_envelope_locked` から `user_queue_lock` の取得を除去する（内部 queue の mutex で十分）
- `Mailbox::dequeue` から `user_queue_lock` の取得を除去する（single consumer + 内部 queue の mutex で十分）
- `Mailbox::user_len` / `Mailbox::publish_metrics` から `user_queue_lock` の取得を除去する（metrics は厳密でなくてよい）
- `user_queue_lock` を `put_lock` に改名し、以下の compound op **のみ** で取得する:
  - `prepend_user_messages_deque_locked`（deque prepend の atomicity 保証）
  - `become_closed_and_clean_up`（close + drain の直列化）
- Pekko の `BoundedControlAwareMailbox.putLock` と同じ責務に限定

## Capabilities

### Modified Capabilities
- `mailbox-enqueue-performance`: 通常 enqueue の lock 段数が 1 段減少
- `mailbox-dequeue-performance`: 通常 dequeue の lock 段数が 1 段減少

## Impact

- 対象コード: `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs`
- 影響内容:
  - 通常 enqueue/dequeue の hot path から Mutex 1 段除去
  - compound op (prepend, close+cleanup) は lock を維持（安全性に影響なし）
  - metrics 読み取りは近似値を許容（中間値が見える可能性あるが致命的でない）
- 非目標:
  - 内側 2 段（QueueStateHandle + SyncQueueShared）の統合（別 change）
  - MessageQueue の lock-free 化（Phase VII）
  - stream-core の同型問題（Phase IX）
