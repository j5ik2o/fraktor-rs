## ADDED Requirements

### Requirement: 通常 enqueue/dequeue は user_queue_lock (put_lock) を取得してはならない

通常の単一 enqueue (`enqueue_envelope_locked`) および単一 dequeue (`dequeue`) は `put_lock` を取得してはならない（MUST NOT）。内部 queue の mutex のみで同期する。

#### Scenario: 通常 enqueue で put_lock を取得しない

- **WHEN** `enqueue_envelope_locked` が呼ばれる
- **THEN** `put_lock` (旧 `user_queue_lock`) の lock 取得は行われない
- **AND** 内部 queue の `enqueue` メソッドのみが呼ばれる
- **AND** close チェックは atomic flag (`is_closed()`) で行われる

#### Scenario: 通常 dequeue で put_lock を取得しない

- **WHEN** `dequeue` が呼ばれる
- **THEN** `put_lock` の lock 取得は行われない
- **AND** 内部 queue の `dequeue` メソッドのみが呼ばれる

### Requirement: compound op は put_lock を取得しなければならない

複数の queue 操作を atomic 化する compound op（prepend、close+cleanup）は `put_lock` を取得しなければならない（MUST）。

#### Scenario: prepend は put_lock で保護される

- **WHEN** `prepend_user_messages_deque_locked` が呼ばれる
- **THEN** `put_lock` を取得してから deque 操作を行う
- **AND** lock 保持中に capacity チェック → 複数 enqueue_first を atomic に実行する

#### Scenario: close+cleanup は put_lock で保護される

- **WHEN** `become_closed_and_clean_up` が呼ばれる
- **THEN** `put_lock` を取得してから drain を行う
- **AND** close flag 立て → drain が直列化される

### Requirement: close 後のメッセージは失われてはならない

close race window（`is_closed()` チェック → `user.enqueue()` の間に close が起きる）で到着したメッセージは、drain で dead letters に転送されなければならない（MUST）。

#### Scenario: close 直前の enqueue はメッセージを失わない

- **GIVEN** producer が `is_closed()` チェックを通過した直後に close が実行される
- **WHEN** producer が `user.enqueue(envelope)` を完了する
- **THEN** `become_closed_and_clean_up` の drain でそのメッセージが回収される
- **AND** dead letters に転送される
