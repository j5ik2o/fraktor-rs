# mailbox-close-semantics Specification

## Purpose
TBD - created by archiving change mailbox-close-reject-enqueue. Update Purpose after archive.
## Requirements
### Requirement: Mailbox cleanup SHALL serialize close with mailbox-owned user queue mutation

`Mailbox::become_closed_and_clean_up` は、mailbox-owned な user queue mutation (`enqueue_envelope`, `enqueue_user`, `prepend_user_messages`) と close を直列化するために、**`user_queue_lock` を取得してから `state.close()` を実行しなければならない**（MUST）。

同じ lock 区間の中で以下を完了しなければならない（MUST）:

- `state.close()`
- 必要なら user queue の drain
- `self.user.clean_up()`

これにより、cleanup が close を宣言した mailbox に対して、待機していた producer / unstash 側が lock 解放後に queue mutation を完了してしまう race を防ぐ。

#### Scenario: become_closed_and_clean_up closes under user_queue_lock
- **WHEN** `Mailbox::become_closed_and_clean_up` の実装を確認する
- **THEN** `user_queue_lock` の取得が `state.close()` より前にある
- **AND** `state.close()` の後に実行される user queue の drain / `clean_up()` は同じ lock 区間に含まれる

### Requirement: enqueue_envelope SHALL re-check closed state after acquiring user_queue_lock

`Mailbox::enqueue_envelope` は、fast path の `is_closed()` / `is_suspended()` check に加えて、**`user_queue_lock` 取得後に `is_closed()` を再 check** しなければならない（MUST）。

lock 内再 check が true の場合、メソッドは `Err(SendError::Closed(envelope.into_payload()))` を返さなければならない（MUST）。`self.user.enqueue(envelope)` は実行してはならない（MUST NOT）。

#### Scenario: Sequential enqueue after close returns Closed
- **WHEN** mailbox を作成し `become_closed_and_clean_up()` を呼ぶ
- **AND** `enqueue_envelope(Envelope::new(AnyMessage::new("msg")))` を呼ぶ
- **THEN** 結果は `Err(SendError::Closed(_))` である

#### Scenario: In-flight enqueue is rejected after cleanup wins the lock race
- **WHEN** producer が fast path を通過した後、cleanup 側が `user_queue_lock` を取得して `state.close()` と queue cleanup を完了する
- **AND** その後 producer が `user_queue_lock` を取得する
- **THEN** producer は lock 内の `is_closed()` 再 check で `Err(SendError::Closed(_))` を返す
- **AND** `self.user.enqueue(envelope)` は呼ばれない

### Requirement: enqueue_user SHALL inherit the same close-rejection contract

`Mailbox::enqueue_user` は `enqueue_envelope` の薄い wrapper として、close 後に `Err(SendError::Closed(message))` を返さなければならない（MUST）。

#### Scenario: enqueue_user after close returns Closed
- **WHEN** mailbox を close した後に `enqueue_user(AnyMessage::new("msg"))` を呼ぶ
- **THEN** 結果は `Err(SendError::Closed(_))` である

#### Scenario: Closed takes precedence over Suspended on user enqueue paths
- **WHEN** mailbox が `closed` と `suspended` の両方を満たす状態で user message enqueue 系 API を呼ぶ
- **THEN** 返るエラーは `SendError::Suspended` ではなく `SendError::Closed` である

### Requirement: prepend_user_messages SHALL re-check closed state after acquiring user_queue_lock

`Mailbox::prepend_user_messages` は production reachable な user message mutation path として、fast path の `is_closed()` / `is_suspended()` check に加えて、**`user_queue_lock` 取得後に `is_closed()` を再 check** しなければならない（MUST）。

lock 内再 check が true の場合、`SendError::Closed(first_message.clone())` を返し、prepend / drain-and-requeue / queue mutation を実行してはならない（MUST NOT）。

#### Scenario: Sequential prepend after close returns Closed
- **WHEN** mailbox を close した後に `prepend_user_messages(&messages)` を呼ぶ
- **THEN** 結果は `Err(SendError::Closed(_))` である

#### Scenario: In-flight prepend is rejected after cleanup wins the lock race
- **WHEN** prepend 側が fast path を通過した後、cleanup 側が `user_queue_lock` を取得して `state.close()` と queue cleanup を完了する
- **AND** その後 prepend 側が `user_queue_lock` を取得する
- **THEN** prepend 側は lock 内の `is_closed()` 再 check で `Err(SendError::Closed(_))` を返す
- **AND** prepend による queue mutation は起こらない

### Requirement: Mailbox::is_closed SHALL remain true after become_closed_and_clean_up

`Mailbox::is_closed()` は `become_closed_and_clean_up()` 後に true を返し続けなければならない（MUST）。

#### Scenario: is_closed transitions to true
- **WHEN** mailbox を新規作成した直後
- **THEN** `is_closed()` は false
- **WHEN** `become_closed_and_clean_up()` を呼ぶ
- **THEN** `is_closed()` は true

