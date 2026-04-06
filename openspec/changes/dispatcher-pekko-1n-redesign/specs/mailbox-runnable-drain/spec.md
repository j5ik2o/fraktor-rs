## ADDED Requirements

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

mailbox は message queue を外部から注入可能なコンストラクタを持ち、将来の `BalancingDispatcher` が shared queue を差し込めるようにしなければならない (MUST)。

#### Scenario: Mailbox::new は queue を引数に取る
- **WHEN** `Mailbox::new(actor, queue)` のシグネチャを確認する
- **THEN** `queue` は外部から構築された `ArcShared<Box<dyn MessageQueue>>`（またはそれに相当する既存の `MessageQueueShared` 相当型）で受け取る
- **AND** mailbox が message queue を内部で new する経路のみに限定されていない

#### Scenario: MessageQueue trait は multi-consumer を許容する
- **WHEN** `MessageQueue` trait のシグネチャを確認する
- **THEN** `enqueue` / `dequeue` / `len` などのメソッドは `&self` のみを要求する
- **AND** `&mut self` を要求するメソッドは存在しない
- **AND** これは将来 `TeamQueueMessageQueue`（multi-consumer）を実装するための seam となる
