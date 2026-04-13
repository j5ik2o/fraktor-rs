## 1. `Mailbox` close 順序の修正

`become_closed_and_clean_up` が cleanup policy に関わらず `user_queue_lock` を取得してから `state.close()` し、同じ lock 区間で user queue の drain / clean_up を完了するように変更する。

- [x] 1.1 `modules/actor-core/src/core/kernel/dispatch/mailbox/base.rs` の `become_closed_and_clean_up` で、cleanup policy の分岐に関わらず `user_queue_lock` 取得を `state.close()` より前に移動する
- [x] 1.2 `DrainToDeadLetters` の drain を同じ lock 区間で実行する
- [x] 1.3 `self.user.clean_up()` も同じ lock 区間で実行する
- [x] 1.4 `MailboxCleanupPolicy::LeaveSharedQueue` の sharing mailbox でも、`lock -> close -> clean_up` の順序を維持する
- [x] 1.5 cleanup 後の `user_len` snapshot を lock 区間内で取得し、`publish_metrics_with_user_len(...)` は lock 解放後に呼ぶ形で実装する
- [x] 1.6 `self.user.clean_up()` を lock 内に移す前提として、`MessageQueue` 実装 (`UnboundedMessageQueue`, `BoundedMessageQueue`, `BoundedPriorityMessageQueue`, `BoundedStablePriorityMessageQueue`, `UnboundedPriorityMessageQueue`, `UnboundedStablePriorityMessageQueue`, `UnboundedDequeMessageQueue`, `UnboundedControlAwareMessageQueue`, `SharedMessageQueueBox`, `ScriptedMessageQueue`) が `Mailbox` 側へ再入せず `user_queue_lock` を再取得しないことを確認する

## 2. `enqueue_envelope` の lock 内 closed 再 check

`enqueue_envelope` は fast path の `is_closed()` / `is_suspended()` に加えて、`user_queue_lock` 取得後に `is_closed()` を再 check する。

- [x] 2.1 `enqueue_envelope` の先頭に `if self.is_closed()` を追加する
- [x] 2.2 `is_closed()` check を `is_suspended()` より前に置く
- [x] 2.3 `user_queue_lock` 取得後に `if self.is_closed()` を再 check する
- [x] 2.4 inner re-check で closed の場合は `SendError::closed(envelope.into_payload())` を返す
- [x] 2.5 既存の enqueue_result / metrics 処理は維持する

## 3. `prepend_user_messages` の lock 内 closed 再 check

`prepend_user_messages` も production reachable な user message mutation path として同じ close correctness を持たせる。

- [x] 3.1 `prepend_user_messages` の空 batch 判定の後に `if self.is_closed()` を追加する
- [x] 3.2 `is_closed()` check を `is_suspended()` より前に置く
- [x] 3.3 `user_queue_lock` 取得後に `if self.is_closed()` を再 check する
- [x] 3.4 closed の場合は `SendError::closed(first_message.clone())` を返す
- [x] 3.5 既存の capacity check / deque prepend / drain-and-requeue 経路は維持する

## 4. 直列テストの追加

`modules/actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs` に直列ケースの contract test を追加する。

- [x] 4.1 `mailbox_enqueue_envelope_returns_closed_after_mailbox_close` を追加する
- [x] 4.2 `mailbox_enqueue_user_returns_closed_after_mailbox_close` を追加する
- [x] 4.3 `mailbox_prepend_user_messages_returns_closed_after_mailbox_close` を追加する
- [x] 4.4 `mailbox_is_closed_after_mailbox_close` を追加する
- [x] 4.5 既存の `mailbox_enqueue_user_returns_closed_when_queue_is_closed` と意図が混線しないよう、必要なら既存または新規テストの命名を整理する

## 5. 並行回帰テストの追加

fast path 通過後に cleanup が close を完了する interleave で phantom enqueue が起きないことを verify する。

- [x] 5.1 `cleanup_close_wins_against_inflight_enqueue` を追加する
  - 目的: producer が fast path を通過済みでも、cleanup が先に close を確立したら `enqueue_user` が `Err(SendError::Closed(_))` になることを verify
  - 推奨手段: `base.rs` に `#[cfg(test)]` の pre-lock hook を追加して deterministic に interleave を固定する
- [x] 5.2 `cleanup_close_wins_against_inflight_prepend` を追加する
  - 目的: in-flight `prepend_user_messages` も同じく closed で reject されることを verify
  - 推奨手段: 同じ pre-lock hook を prepend 経路にも適用し、既存の thread + channel パターンを再利用する

## 6. スコープ外が触られていないことの確認

- [x] 6.1 `enqueue_system` に変更が入っていないことを確認する
- [x] 6.2 `SharedMessageQueue` / `BalancingDispatcher::dispatch` に変更が入っていないことを確認する
- [x] 6.3 `MessageQueue` trait に `close()` / `is_closed()` が追加されていないことを確認する

## 7. 検証

- [x] 7.1 `cargo check -p fraktor-actor-core-rs --lib` clean
- [x] 7.2 `cargo test -p fraktor-actor-core-rs --lib` 全件 pass
- [x] 7.3 `cargo test -p fraktor-actor-core-rs --tests` 全件 pass
- [x] 7.4 `cargo test --workspace --lib` 全件 pass
- [x] 7.5 `./scripts/ci-check.sh ai all` exit 0
- [x] 7.6 `openspec validate mailbox-close-reject-enqueue --strict` valid
