## Context

Mailbox の `user_queue_lock: SharedLock<()>` は全 enqueue/dequeue で取得される barrier lock。調査 A の結果、6 箇所のうち compound op は 3 箇所（enqueue, prepend, finalize_cleanup）で、残り 3 箇所（dequeue, user_len, publish_metrics）は外側 lock 不要。

前提条件（close correctness, stash deque requirement, prepend deque-only 契約）は全て完了済み。

## Goals / Non-Goals

**Goals:**
- dequeue / metrics 読み取りから `user_queue_lock` の取得を除去
- compound op (enqueue+close-check, prepend, close+cleanup) は `put_lock` として維持
- ベンチマークで効果を計測

**Non-Goals:**
- enqueue 側の lock 除去（TOCTOU race のため不可）
- 内側 lock (QueueStateHandle, SyncQueueShared) の統合
- lock-free queue への移行
- `put_lock` 自体の撤廃

## Decisions

### 1. user_queue_lock → put_lock への改名と限定化

`docs/plan/lock-strategy-analysis.md` の案 a2 を部分採用。dequeue / metrics のみ lock 除去:

```rust
// Before: dequeue で取得
pub(crate) fn dequeue(&self) -> Option<MailboxMessage> {
  let result = self.user_queue_lock.with_lock(|_| {
    if self.state.is_close_requested() {
      return None;
    }
    self.user.dequeue().map(MailboxMessage::User)
  });
  ...
}

// After: dequeue では取得しない
pub(crate) fn dequeue(&self) -> Option<MailboxMessage> {
  let result = {
    if self.state.is_close_requested() {
      None
    } else {
      self.user.dequeue().map(MailboxMessage::User)
    }
  };
  ...
}
```

### 2. enqueue 側の lock を維持する理由（TOCTOU race 分析）

`enqueue_envelope_locked` は `is_closed()` チェック + `user.enqueue()` の compound op。lock を外すと以下の race が発生:

```
Producer                          Closer (finalize_cleanup)
────────                          ──────
is_closed() → false
                                  request_close() → FLAG_CLOSE_REQUESTED (lock 外)
                                  finalize_cleanup() acquires put_lock
                                    drain all messages
                                    clean_up()          ← queue リセット
                                    finish_cleanup()    ← FLAG_CLEANUP_DONE
                                  release put_lock
user.enqueue(msg)                 ← phantom enqueue into cleaned queue
```

`clean_up()` 後に enqueue されたメッセージは drain されず失われる。Pekko は lock-free queue（`AbstractNodeQueue`）を使うため drain 後の enqueue は次回 drain で回収されるが、fraktor-rs の queue 実装では保証されない。

したがって、lock-free queue 導入（Phase VII）まで enqueue 側の lock は維持する。

### 3. dequeue の lock 除去の安全性

dequeue は single consumer（dispatcher thread の `run()` ループ、`FLAG_RUNNING` で排他）。

- producer (enqueue) との同期: 内部 queue の mutex で担保
- compound op (prepend) との排他: prepend は内部 queue の deque API (`enqueue_first`) を使うため、内部 mutex で直列化される
- close check (`is_close_requested()`): atomic flag チェック。dequeue 側で TOCTOU があっても、close 後に dequeue が空を返すだけで harm はない

### 4. metrics の近似値許容

`user_len()` / `publish_metrics()` は `user.number_of_messages()` を呼ぶだけ。内部 queue の mutex で一貫性は担保される。compound op 途中の中間値が見える可能性があるが、metrics 目的では許容。

## Risks / Trade-offs

- [Risk] dequeue と prepend の並行実行 → Mitigation: 内部 queue の mutex で直列化される
- [Risk] dequeue と close の並行実行 → Mitigation: close 後に dequeue が空を返すだけ。harm なし
- [Risk] 将来の compound op 追加時に put_lock の取得を忘れる → Mitigation: put_lock のドキュメントに「compound op では必ず取得」を明記

## Open Questions

- なし（前提条件は全て充足済み）
