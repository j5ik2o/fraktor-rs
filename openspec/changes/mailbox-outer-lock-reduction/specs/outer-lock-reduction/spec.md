## ADDED Requirements

### Requirement: dequeue と metrics 読み取りは put_lock を取得してはならない

通常の単一 dequeue (`dequeue`) および metrics 読み取り (`user_len`, `publish_metrics`) は `put_lock` を取得してはならない（MUST NOT）。内部 queue の mutex のみで同期する。

#### Scenario: 通常 dequeue で put_lock を取得しない

- **WHEN** `dequeue` が呼ばれる
- **THEN** `put_lock` (旧 `user_queue_lock`) の lock 取得は行われない
- **AND** 内部 queue の `dequeue` メソッドのみが呼ばれる

#### Scenario: metrics 読み取りで put_lock を取得しない

- **WHEN** `user_len` または `publish_metrics` が呼ばれる
- **THEN** `put_lock` の lock 取得は行われない
- **AND** 内部 queue の `number_of_messages` メソッドのみが呼ばれる

### Requirement: enqueue の compound op は put_lock を取得しなければならない

`enqueue_envelope_locked` は `is_closed()` チェック + `user.enqueue()` の compound op であり、`put_lock` を取得しなければならない（MUST）。lock を外すと `finalize_cleanup` との TOCTOU race で phantom enqueue が発生する。

#### Scenario: enqueue は put_lock で保護される

- **WHEN** `enqueue_envelope_locked` が呼ばれる
- **THEN** `put_lock` を取得してから `is_closed()` チェック + `user.enqueue()` を実行する
- **AND** `finalize_cleanup` の drain + `clean_up()` と直列化される

### Requirement: prepend と close+cleanup の compound op は put_lock を取得しなければならない

複数の queue 操作を atomic 化する compound op（prepend、close+cleanup）は `put_lock` を取得しなければならない（MUST）。

#### Scenario: prepend は put_lock で保護される

- **WHEN** `prepend_user_messages_deque_locked` が呼ばれる
- **THEN** `put_lock` を取得してから deque 操作を行う
- **AND** lock 保持中に `is_closed()` チェック → 複数 `enqueue_first` を atomic に実行する

#### Scenario: finalize_cleanup は put_lock で保護される

- **WHEN** `finalize_cleanup` が呼ばれる
- **THEN** `put_lock` を取得してから drain + `clean_up()` + `finish_cleanup()` を行う
- **AND** enqueue / prepend と直列化される

### Requirement: close 後のメッセージは失われてはならない

enqueue の compound op が `put_lock` で保護されていることで、`is_closed()` が false を返した後に close が起きた場合でも、`finalize_cleanup` の drain で確実に回収される。

#### Scenario: close 直前の enqueue はメッセージを失わない

- **GIVEN** producer が `put_lock` を取得し `is_closed() → false` を確認
- **WHEN** lock 保持中に `user.enqueue(envelope)` を完了する
- **THEN** `finalize_cleanup` は put_lock 取得を待ってから drain する
- **AND** そのメッセージは drain で回収され dead letters に転送される
