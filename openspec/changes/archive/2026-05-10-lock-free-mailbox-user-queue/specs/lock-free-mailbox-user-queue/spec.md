## ADDED Requirements

### Requirement: 通常 unbounded user queue は lock-free MPSC hot path を提供する

通常 unbounded mailbox の user queue は、複数 producer からの enqueue と単一 mailbox runner による dequeue を、user queue 内部の `SharedLock` / `SharedRwLock` / mutex に依存せずに実行しなければならない (MUST)。この queue は `MessageQueue` trait と `Envelope` payload contract を維持しなければならない (MUST)。

#### Scenario: 通常 unbounded mailbox は lock-free queue を使う
- **WHEN** `MailboxPolicy::unbounded(None)` から通常 mailbox の user queue を作成する
- **THEN** 作成される user queue は lock-free MPSC-backed FIFO queue である
- **AND** enqueue / dequeue hot path は user queue 内部の `SharedLock` / `SharedRwLock` / mutex を取得しない

#### Scenario: lock-free 化対象外の queue は既存 semantics を維持する
- **WHEN** bounded / priority / stable-priority / control-aware / deque-prepend capable queue を作成する
- **THEN** それらの queue は本 change で lock-free MPSC queue に置き換えてはならない (MUST NOT)
- **AND** 既存の overflow / priority / control-aware / prepend semantics を維持しなければならない (MUST)

### Requirement: lock-free user queue は FIFO と exact-once dequeue を保証する

lock-free user queue は、enqueue に成功した `Envelope` を最大 1 回だけ dequeue しなければならない (MUST)。同一 producer が enqueue した envelopes は、producer 内の enqueue 成功順で dequeue されなければならない (MUST)。

#### Scenario: single producer の FIFO が維持される
- **WHEN** 1 producer が複数の envelopes を順番に enqueue する
- **AND** mailbox runner が queue を drain する
- **THEN** dequeue される envelopes は enqueue 成功順と同じ順序である
- **AND** 各 envelope は 1 回だけ dequeue される

#### Scenario: multiple producers の envelopes が loss/duplicate しない
- **WHEN** 複数 producer が並行して envelopes を enqueue する
- **AND** mailbox runner が queue を drain する
- **THEN** enqueue に成功したすべての envelopes が 1 回だけ dequeue される
- **AND** 同一 producer 内の envelopes は producer 内の enqueue 成功順を維持する

### Requirement: lock-free user queue は queue-local atomic close protocol を持つ

lock-free user queue は queue-local な atomic close protocol を持ち、close が publish された後の enqueue を拒否しなければならない (MUST)。close/cleanup は in-flight producer が完了するまで待ってから残留 envelopes を drain しなければならない (MUST)。

#### Scenario: close 後 enqueue は Closed を返す
- **WHEN** lock-free user queue を close する
- **AND** その後 producer が envelope を enqueue する
- **THEN** enqueue は `Closed` 相当の error を返す
- **AND** envelope は queue に残ってはならない (MUST NOT)

#### Scenario: close と競合した in-flight enqueue は lost message を作らない
- **WHEN** producer が enqueue protocol に入った後に close が publish される
- **THEN** producer は close を観測して enqueue を拒否するか、enqueue 成功済み envelope として cleanup drain に観測されなければならない (MUST)
- **AND** cleanup 完了後に queue 内へ envelope が残ってはならない (MUST NOT)

### Requirement: unsafe は lock-free queue primitive 内に局所化される

lock-free user queue の unsafe code は queue primitive module 内に局所化され、public API は safe Rust API として提供されなければならない (MUST)。raw pointer ownership と atomic interleaving は `miri` および `loom` で検証可能でなければならない (MUST)。

#### Scenario: unsafe block は primitive module の外に漏れない
- **WHEN** lock-free user queue の実装を確認する
- **THEN** raw pointer dereference / `Box::from_raw` / node unlink に関する unsafe block は queue primitive module 内に限られる
- **AND** `MessageQueue` 実装や mailbox call site は unsafe を直接呼び出してはならない (MUST NOT)

#### Scenario: verification task が primitive を検証する
- **WHEN** lock-free user queue の検証タスクを実行する
- **THEN** raw pointer ownership safety は `miri` で検証される
- **AND** producer/consumer interleaving は `loom` model test で検証される
