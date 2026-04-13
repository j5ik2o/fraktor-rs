## Context

Mailbox の `user_queue_lock: SharedLock<()>` は全 enqueue/dequeue で取得される barrier lock。調査 A の結果、7 箇所のうち compound op は 2 箇所のみで、残り 5 箇所は外側 lock 不要。Pekko の `putLock` は compound op 時のみ取得する設計であり、これに準拠する。

前提条件（close correctness, stash deque requirement, prepend deque-only 契約）は全て完了済み。

## Goals / Non-Goals

**Goals:**
- 通常 enqueue/dequeue/read から `user_queue_lock` の取得を除去
- compound op (prepend, close+cleanup) のみ `put_lock` として取得
- ベンチマークで効果を計測

**Non-Goals:**
- 内側 lock (QueueStateHandle, SyncQueueShared) の統合
- lock-free queue への移行
- `put_lock` 自体の撤廃

## Decisions

### 1. user_queue_lock → put_lock への改名と限定化（案 a2 採用）

`docs/plan/lock-strategy-analysis.md` の案 a2 を採用:

```rust
// Before: 全 enqueue/dequeue で取得
pub(crate) fn enqueue_envelope_locked(&self, envelope: Envelope) -> Result<(), SendError> {
  self.user_queue_lock.with_lock(|_| {
    if self.is_closed() { return Err(...); }
    self.user.enqueue(envelope)
  })
}

// After: 通常 enqueue では取得しない
pub(crate) fn enqueue_envelope_locked(&self, envelope: Envelope) -> Result<(), SendError> {
  if self.is_closed() { return Err(SendError::closed(...)); }
  self.user.enqueue(envelope)
}
```

### 2. close correctness の維持

`enqueue_envelope_locked` から lock を外すと、`become_closed_and_clean_up` との race が理論上起きる。しかし:

1. `is_closed()` は atomic flag チェック（lock 不要）
2. `become_closed_and_clean_up` は `put_lock` を取得し、close flag を立ててから drain する
3. close flag が立った後の enqueue は `is_closed()` で弾かれる
4. flag 立て → drain の間に到着したメッセージは drain で回収される

ただし `is_closed()` チェックと `user.enqueue()` の間に close が起きると、enqueue 成功後に drain が走る。これは Pekko でも同じ挙動（atomic flag ベースなので同一の race window が存在）であり、許容される。drain がメッセージを dead letters に転送するため、メッセージは失われない。

### 3. dequeue の lock 除去

dequeue は single consumer (dispatcher thread の `run()` ループ)。producer (enqueue) との同期は内部 queue の mutex で担保される。compound op (prepend) との排他は `put_lock` で担保される。したがって dequeue 側で `user_queue_lock` を取る必要はない。

ただし `prepend_user_messages_deque_locked` が dequeue と同時に走る可能性がある。prepend は `put_lock` を取得するが、dequeue は取得しない場合、prepend 中に dequeue が割り込む。

対策: `prepend_user_messages_deque_locked` は **deque の内部 lock** を取得して操作するため、dequeue と prepend は内部 lock で直列化される。外側 lock は不要。

### 4. metrics の近似値許容

`user_len()` / `publish_metrics()` は `user.number_of_messages()` を呼ぶだけ。内部 queue の mutex で一貫性は担保される。compound op 途中の中間値が見える可能性があるが、metrics 目的では許容。

## Risks / Trade-offs

- [Risk] close race window でメッセージが close 後に enqueue される → Mitigation: drain が dead letters に転送するため失われない。Pekko と同じ挙動
- [Risk] prepend と dequeue の並行実行 → Mitigation: 内部 queue の mutex で直列化される
- [Risk] 将来の compound op 追加時に put_lock の取得を忘れる → Mitigation: put_lock のドキュメントに「compound op では必ず取得」を明記

## Open Questions

- なし（前提条件は全て充足済み）
