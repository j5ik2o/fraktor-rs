# mailbox-runnable-drain Specification

## Purpose
TBD - created by archiving change dispatcher-pekko-1n-redesign. Update Purpose after archive.
## Requirements
### Requirement: mailbox は自ら drain ループを所有する

mailbox は Pekko の `Mailbox extends Runnable` と同じく、自身の `run()` に drain ループ本体を所有しなければならない (MUST)。dispatcher は mailbox を executor に submit する経路のみを提供する。

#### Scenario: Mailbox::run は system message を全件処理してから user message を throughput まで処理する
- **WHEN** `Mailbox::run(throughput, throughput_deadline)` が実行される
- **THEN** まず system message を全件 drain する
- **AND** 次に user message を `throughput` で指定された数まで処理する
- **AND** `throughput_deadline` が `Some` の場合、その時間内に処理を打ち切る
- **AND** mailbox が closed なら即 return する
- **AND** mailbox が suspended な場合は system message のみ処理する

#### Scenario: drain ループ本体は dispatcher 側に存在しない
- **WHEN** dispatcher の実装を確認する
- **THEN** `DispatcherCore` / `DefaultDispatcher` / `PinnedDispatcher` のいずれも drain ループ本体（`process_batch` 相当）を持たない
- **AND** dispatcher の責務は `register_for_execution` で mailbox を executor に submit することに限られる

#### Scenario: Mailbox は dispatcher への参照を持たない
- **WHEN** `Mailbox` の field 構造を確認する
- **THEN** mailbox は `MessageDispatcherShared` や `ArcShared<MessageDispatcher>` / `ArcShared<DispatcherCore>` を field として保持しない
- **AND** drain 後の再スケジュール経路は、`MessageDispatcherShared::register_for_execution` が `ExecutorShared::execute` に submit する closure 内で `self.clone()` した `MessageDispatcherShared` をキャプチャする形で提供される

### Requirement: Mailbox コンストラクタは message queue を外部注入できる

mailbox は message queue を外部から注入可能なコンストラクタを持ち、`BalancingDispatcher` の `SharingMailbox` が shared queue を差し込めるようにしなければならない (MUST)。

#### Scenario: Mailbox::new は queue を引数に取る
- **WHEN** `Mailbox::new(actor: Weak<ActorCell>, queue: ArcShared<dyn MessageQueue>)` のシグネチャを確認する
- **THEN** `actor` は `Weak<ActorCell>` で、circular reference 回避とライフサイクル分離のため
- **AND** `queue` は外部から構築された `ArcShared<Box<dyn MessageQueue>>`（またはそれに相当する型）で受け取る
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
- **AND** `Mailbox::new_sharing(actor: Weak<ActorCell>, shared_queue: ArcShared<SharedMessageQueue>)` が存在する
- **AND** `Mailbox` は `MailboxCleanupPolicy::LeaveSharedQueue` を保持できる
- **AND** `run()` の挙動は通常 Mailbox と同じ（system 全件 → user throughput まで dequeue）
- **AND** **`clean_up()` の挙動だけ通常 Mailbox と異なる**: `LeaveSharedQueue` の場合は shared queue を drain しない (queue は他の team member が引き続き使用するため)
- **AND** これらの実装は core 層に置かれる

#### Scenario: `Mailbox::new_sharing(...)` は BalancingDispatcher 以外で使われない
- **WHEN** `Mailbox::new_sharing(...)` の利用箇所を確認する
- **THEN** `BalancingDispatcher::create_mailbox` 以外で呼ばれていない
- **AND** `DefaultDispatcher` / `PinnedDispatcher` は通常 Mailbox を使用する
