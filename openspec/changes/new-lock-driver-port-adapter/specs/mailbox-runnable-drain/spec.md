## MODIFIED Requirements

### Requirement: Mailbox コンストラクタは message queue を外部注入できる

mailbox は message queue を外部から注入可能なコンストラクタを持ち、`BalancingDispatcher` の `SharingMailbox` が shared queue を差し込めるようにしなければならない (MUST)。加えて、mailbox は actor runtime lock provider が生成した `MailboxLockSet` を同一の constructor で受け取らなければならない (MUST)。

#### Scenario: Mailbox::new は queue と lock set を引数に取る
- **WHEN** `Mailbox::new(actor: Weak<ActorCell>, queue: ArcShared<dyn MessageQueue>, lock_set: MailboxLockSet)` のシグネチャを確認する
- **THEN** `actor` は `Weak<ActorCell>` で、circular reference 回避とライフサイクル分離のため
- **AND** `queue` は外部から構築された `ArcShared<Box<dyn MessageQueue>>`（またはそれに相当する型）で受け取る
- **AND** `lock_set` は actor runtime lock provider から同一 family で取得された `MailboxLockSet` を受け取る
- **AND** mailbox が message queue を内部で new する経路のみに限定されていない
- **AND** `Mailbox::run()` は `Weak::upgrade()` で actor を取得し、None なら early return する

#### Scenario: MessageQueue trait は Envelope を運搬し multi-consumer を許容する
- **WHEN** `MessageQueue` trait のシグネチャを確認する
- **THEN** user queue の payload 型は `Envelope` に統一されている
- **AND** `enqueue` / `dequeue` / `len` などのメソッドは `&self` のみを要求する
- **AND** `&mut self` を要求するメソッドは存在しない
- **AND** これは `SharedMessageQueue` (multi-consumer) を実装するための seam として使われる
- **AND** `SharedMessageQueue` だけ別 payload 契約を持つことはない

#### Scenario: Envelope は dispatch/mailbox 層の薄い AnyMessage wrapper として定義される
- **WHEN** `Envelope` 型の定義を確認する
- **THEN** `Envelope` は actor-core の dispatch/mailbox 層に置かれている
- **AND** 最小フィールドは `payload: AnyMessage` のみである
- **AND** sender 等の既存メタデータは `AnyMessage` 側に保持された値を再利用する
- **AND** receiver / priority / correlation_id 等の追加フィールドは本 change では要求しない

### Requirement: SharedMessageQueue と「SharingMailbox 概念」は BalancingDispatcher 用に core 層に提供される

`BalancingDispatcher` の load balancing を実現するため、`SharedMessageQueue` と「SharingMailbox 概念」は core 層 (`no_std` 対応) に置かれなければならない (MUST)。ここでいう `SharingMailbox` は独立した struct 名ではなく、`Mailbox::new_sharing(...)` で構築され `MailboxCleanupPolicy::LeaveSharedQueue` を持つ `Mailbox` instance の概念名である。これらは `BalancingDispatcher` の internal detail であり、他の dispatcher からは使用してはならない (MUST NOT)。

#### Scenario: SharedMessageQueue は thread-safe な multi-consumer queue である
- **WHEN** `SharedMessageQueue` の定義を確認する
- **THEN** `pub struct SharedMessageQueue { inner: ArcShared<RuntimeMutex<VecDeque<Envelope>>> }` 等の thread-safe な内部構造を持つ
- **AND** `MessageQueue` trait を実装し、`enqueue` / `dequeue` / `len` / `is_empty` がすべて `&self` シグネチャ
- **AND** core 層 (`modules/actor-core/src/core/kernel/dispatch/dispatcher_new/shared_message_queue.rs`) に置かれる
- **AND** `no_std` 対応である
- **AND** 後で lock-free 実装に差し替え可能なシグネチャに留められている

#### Scenario: SharingMailbox 概念は `Mailbox::new_sharing(...)` と cleanup policy で表現される
- **WHEN** BalancingDispatcher 用 mailbox の定義を確認する
- **THEN** 独立した `SharingMailbox` struct は存在しない
- **AND** `Mailbox::new_sharing(actor: Weak<ActorCell>, shared_queue: ArcShared<SharedMessageQueue>, lock_set: MailboxLockSet)` が存在する
- **AND** `lock_set` は actor runtime lock provider から同一 family で取得された `MailboxLockSet` を受け取る
- **AND** `Mailbox` は `MailboxCleanupPolicy::LeaveSharedQueue` を保持できる
- **AND** `run()` の挙動は通常 Mailbox と同じ（system 全件 → user throughput まで dequeue）
- **AND** **`clean_up()` の挙動だけ通常 Mailbox と異なる**: `LeaveSharedQueue` の場合は shared queue を drain しない (queue は他の team member が引き続き使用するため)
- **AND** これらの実装は core 層に置かれる

#### Scenario: `Mailbox::new_sharing(...)` は BalancingDispatcher 以外で使われない
- **WHEN** `Mailbox::new_sharing(...)` の利用箇所を確認する
- **THEN** `BalancingDispatcher::create_mailbox` 以外で呼ばれていない
- **AND** `DefaultDispatcher` / `PinnedDispatcher` は通常 Mailbox を使用する
