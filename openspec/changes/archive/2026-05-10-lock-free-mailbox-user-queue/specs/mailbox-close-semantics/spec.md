## MODIFIED Requirements

### Requirement: Mailbox cleanup SHALL serialize close with mailbox-owned user queue mutation

`Mailbox::become_closed_and_clean_up` は、mailbox-owned な lock-backed user queue mutation (`enqueue_envelope`, `enqueue_user`, `prepend_user_messages`) と close を直列化するために、**`user_queue_lock` / `put_lock` を取得してから `state.close()` 相当の close request を publish しなければならない**（MUST）。

通常 lock-free user queue の場合、`Mailbox::become_closed_and_clean_up` は `user_queue_lock` / `put_lock` ではなく queue-local atomic close protocol によって enqueue と close を直列化しなければならない（MUST）。この場合、close request を publish した後、in-flight producer が完了するまで待ち、残留 user queue を drain / `clean_up()` しなければならない（MUST）。

同じ close/cleanup protocol の中で以下を完了しなければならない（MUST）:

- close request の publish
- 必要なら user queue の drain
- `self.user.clean_up()`

これにより、cleanup が close を宣言した mailbox に対して、待機中または in-flight の producer / unstash 側が cleanup 完了後に queue mutation を完了してしまう race を防ぐ。

#### Scenario: lock-backed queue cleanup closes under user_queue_lock
- **WHEN** lock-backed user queue を持つ mailbox の cleanup 実装を確認する
- **THEN** `user_queue_lock` / `put_lock` の取得が close request publish より前にある
- **AND** close request publish の後に実行される user queue の drain / `clean_up()` は同じ lock 区間に含まれる

#### Scenario: lock-free queue cleanup closes with queue-local protocol
- **WHEN** lock-free user queue を持つ mailbox の cleanup 実装を確認する
- **THEN** cleanup は通常 enqueue path の `user_queue_lock` / `put_lock` 取得に依存してはならない（MUST NOT）
- **AND** queue-local atomic close protocol が close request を publish する
- **AND** in-flight producer 完了後に残留 user queue の drain / `clean_up()` を行う

### Requirement: enqueue_envelope SHALL re-check closed state after acquiring user_queue_lock

`Mailbox::enqueue_envelope` は、lock-backed user queue に対しては fast path の `is_closed()` / `is_suspended()` check に加えて、**`user_queue_lock` / `put_lock` 取得後に `is_closed()` を再 check** しなければならない（MUST）。

通常 lock-free user queue に対しては、`Mailbox::enqueue_envelope` は通常 enqueue path で `user_queue_lock` / `put_lock` を取得してはならない（MUST NOT）。代わりに、lock-free user queue の atomic close protocol が close 後 enqueue を拒否しなければならない（MUST）。

lock-backed queue の lock 内再 check が true の場合、メソッドは `Err(SendError::Closed(envelope.into_payload()))` を返さなければならない（MUST）。`self.user.enqueue(envelope)` は実行してはならない（MUST NOT）。

lock-free queue の atomic close protocol が close を観測した場合、メソッドは `Err(SendError::Closed(envelope.into_payload()))` 相当を返さなければならない（MUST）。envelope は cleanup 完了後の queue に残ってはならない（MUST NOT）。

#### Scenario: Sequential enqueue after close returns Closed
- **WHEN** mailbox を作成し `become_closed_and_clean_up()` を呼ぶ
- **AND** `enqueue_envelope(Envelope::new(AnyMessage::new("msg")))` を呼ぶ
- **THEN** 結果は `Err(SendError::Closed(_))` である

#### Scenario: In-flight enqueue is rejected after cleanup wins the lock race
- **WHEN** lock-backed queue の producer が fast path を通過した後、cleanup 側が `user_queue_lock` / `put_lock` を取得して close request と queue cleanup を完了する
- **AND** その後 producer が `user_queue_lock` / `put_lock` を取得する
- **THEN** producer は lock 内の `is_closed()` 再 check で `Err(SendError::Closed(_))` を返す
- **AND** `self.user.enqueue(envelope)` は呼ばれない

#### Scenario: In-flight lock-free enqueue is rejected or drained after cleanup wins close
- **WHEN** lock-free queue の producer が enqueue protocol に入った後、cleanup 側が queue-local close protocol を publish する
- **THEN** producer は `Err(SendError::Closed(_))` を返すか、enqueue 成功済み envelope として cleanup drain に観測される
- **AND** cleanup 完了後にその envelope が user queue 内へ残ってはならない（MUST NOT）
