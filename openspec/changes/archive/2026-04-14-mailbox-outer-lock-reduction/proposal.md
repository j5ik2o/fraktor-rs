## Why

`docs/plan/lock-strategy-analysis.md` の調査 A で判明した通り、Mailbox の `user_queue_lock` は **全ての enqueue/dequeue で取得されている** が、本来は compound op（enqueue+close-check、prepend、close+cleanup）との排他のためだけに存在する。

現状のロック段数:
- `BoundedMessageQueue` 経路: **3 段** (user_queue_lock → QueueStateHandle → SyncQueueShared)
- `UnboundedDequeMessageQueue` 経路: **2 段** (user_queue_lock → inner mutex)

参照実装（Pekko / protoactor-go）は通常 enqueue で **0 段**（lock-free queue ベース）。

dequeue / metrics 読み取りから `user_queue_lock` を外すことで:
- dequeue 側: Bounded 3 → 2 段、Unbounded 2 → 1 段
- metrics 読み取り: lock-free 化

に削減できる。

### enqueue 側の lock は維持する理由

`enqueue_envelope_locked` は `is_closed()` チェック + `user.enqueue()` を compound op として `user_queue_lock` で保護している。この lock を外すと TOCTOU race が発生する:

1. producer が `is_closed() → false` を確認
2. closer が `request_close()` で `FLAG_CLOSE_REQUESTED` を立てる（lock 外）
3. closer が `finalize_cleanup()` で lock を取得し、drain + `clean_up()` + `finish_cleanup()` を実行
4. producer が `user.enqueue(msg)` を実行 → **cleaned queue への phantom enqueue**

Pekko は lock-free queue を使うため drain 後の enqueue が次回 drain で回収されるが、fraktor-rs の現在の queue 実装では `clean_up()` 後に enqueue されたメッセージは回収されない。したがって enqueue 側の lock は維持する。

## 前提条件（全て充足済み）

| 前提 | change | 状態 |
|---|---|---|
| close 時の is_closed 再チェック | `mailbox-close-reject-enqueue` | archive 済み |
| stash が deque mailbox を要求 | `stash-requires-deque-mailbox` | archive 済み |
| prepend が deque-only 契約 | `mailbox-prepend-requires-deque` | archive 済み |
| `prepend_via_drain_and_requeue` 削除 | 同上 | archive 済み |

## What Changes

- `user_queue_lock` を `put_lock` に改名し、Pekko の `BoundedControlAwareMailbox.putLock` と同じ責務に限定
- 以下の **3 箇所** から `put_lock` の取得を除去:
  - `Mailbox::dequeue`（single consumer — `FLAG_RUNNING` で保護、内部 queue の mutex で十分）
  - `Mailbox::user_len`（metrics 読み取り — 近似値許容、内部 queue の mutex で十分）
  - `Mailbox::publish_metrics`（同上）
- 以下の **3 箇所** は `put_lock` を維持:
  - `enqueue_envelope_locked`（`is_closed()` + `enqueue` の compound op — TOCTOU race 防止）
  - `prepend_user_messages_deque_locked`（`is_closed()` + O(k) prepend の compound op）
  - `finalize_cleanup`（drain + `clean_up()` + `finish_cleanup()` の serialization anchor）

## Capabilities

### Modified Capabilities
- `mailbox-dequeue-performance`: dequeue の lock 段数が 1 段減少
- `mailbox-metrics-performance`: metrics 読み取りが lock-free 化

## Impact

- 対象コード: `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs`
- 影響内容:
  - dequeue の hot path から Mutex 1 段除去
  - metrics 読み取りから Mutex 除去
  - enqueue / compound op (prepend, close+cleanup) は lock を維持（安全性に影響なし）
- 非目標:
  - enqueue 側の lock 除去（TOCTOU race のため現時点では不可）
  - 内側 2 段（QueueStateHandle + SyncQueueShared）の統合（別 change）
  - MessageQueue の lock-free 化（Phase VII — lock-free queue 導入後に enqueue 側 lock も除去可能）
  - stream-core の同型問題（Phase IX）
