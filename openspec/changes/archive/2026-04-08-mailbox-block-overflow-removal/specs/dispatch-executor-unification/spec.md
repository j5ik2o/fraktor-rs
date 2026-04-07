## REMOVED Requirements

### Requirement: `DispatcherWaker` は core 層に 1 実装で提供される

`MailboxOfferFuture` を中心とする async backpressure 機能を撤去するため、本 requirement は維持しない。

撤去理由:

- 当該 Waker の唯一の production caller だった `DispatcherSender::drive_offer_future` は PR #1525 (`6ce9b357 fix(dispatcher): address Bugbot review findings on PR #1525`) で busy-loop 指摘 (`Cursor Bugbot #3043806318`) を受けて削除された
- 残った `dispatcher_waker` モジュールは production caller ゼロの dead code となり、Cursor Bugbot から `dispatcher_waker is dead code` 指摘を受けた
- backpressure を提供するための `MailboxOverflowStrategy::Block` 自体が production caller ゼロで、Apache Pekko 自身が非推奨にしている (`Mailboxes.scala:259-263` で `pushTimeOut > 0` 設定に対する warn を出力)。Proto.Actor Go には対応する semantics が存在しない (`actor/bounded.go` の `Bounded` / `BoundedDropping` のいずれも thread parking しない)
- 本 change は当該 Waker、`MailboxOfferFuture`、`EnqueueOutcome::Pending`、`MailboxOverflowStrategy::Block` をまとめて撤去する

`dispatcher_waker.rs` モジュール、`DispatcherWaker` 型、`dispatcher_waker(...) -> Waker` 関数、関連 unit test (`dispatcher_waker/tests.rs`) はすべて削除される。`Waker` 生成経路は他に存在しないため、core 層からは `core::task::Waker` を構築するコードが消える。

## MODIFIED Requirements

### Requirement: `Executor` trait は CQS 準拠の internal primitive として再定義される

dispatcher の内部で使われる executor 抽象は、CQS 準拠の単一 trait として再定義されなければならない (MUST)。command メソッドは `&mut self`、query メソッドは `&self` を要求する。executor を共有する経路は `ExecutorShared`（AShared パターン）を通じてのみ提供され、「queue + mutex + running atomic」のような共有のための再発明は存在してはならない (MUST NOT)。

#### Scenario: Executor trait は CQS 準拠のシグネチャを持つ
- **WHEN** `Executor` trait のシグネチャを確認する
- **THEN** command: `fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError>` が定義されている
- **AND** command: `fn shutdown(&mut self)` が定義されている
- **AND** command を `&self` + 内部可変性で偽装する実装は存在しない
- **AND** `ExecuteError` が executor submit 失敗を表す型として定義されている
- **AND** `supports_blocking()` のような mailbox blocking 互換 query method は存在しない (`MailboxOverflowStrategy::Block` 撤去に伴い不要)

#### Scenario: ExecutorShared は AShared パターンに従う
- **WHEN** `ExecutorShared` の定義を確認する
- **THEN** `ExecutorShared` は `pub struct` として公開されている
- **AND** 内部に `ArcShared<RuntimeMutex<Box<dyn Executor>>>` を保持する
- **AND** `Clone` を実装する（`ArcShared::clone` ベース）
- **AND** `SharedAccess<Box<dyn Executor>>` を実装し、`with_read` / `with_write` を提供する
- **AND** convenience メソッド `execute(&self, task) -> Result<(), ExecuteError>` / `shutdown(&self)` を提供する
- **AND** `supports_blocking()` convenience method は提供しない (trait method ごと撤去)
- **AND** 既存の AShared 系 (`ActorFactoryShared` など) と同じパターンに従っている

#### Scenario: ExecutorShared::execute はロック区間内で task 本体を同期実行しない
- **WHEN** `ExecutorShared::execute(&self, task)` の契約を確認する
- **THEN** `ExecutorShared` は task を executor backend へ submit するだけである
- **AND** `RuntimeMutex` のロック区間内で task 本体を同期実行してはならない
- **AND** submit 完了後にロックを解放し、その後の task 実行は backend 側の責務である

#### Scenario: submit 失敗は ExecuteError として観測される
- **WHEN** executor backend が task submit を拒否する
- **THEN** `Executor::execute` / `ExecutorShared::execute` は `Err(ExecuteError)` を返す
- **AND** 呼び出し側はこの失敗を握りつぶさず、rollback または記録を行う

#### Scenario: DispatchExecutorRunner は存在しない
- **WHEN** `core::kernel::dispatch` 配下を確認する
- **THEN** `DispatchExecutorRunner` および同等の serializing runner は存在しない
- **AND** executor を共有するために `Mutex<Box<dyn ...>>` + internal task queue + `AtomicBool running` を独自に組んだ型は存在しない
- **AND** 複数所有者間の共有は `ExecutorShared` の `ArcShared<RuntimeMutex<Box<dyn Executor>>>` のみで達成される

#### Scenario: Executor trait は core 層に置かれる
- **WHEN** `Executor` trait の定義ファイルを確認する
- **THEN** `Executor` trait は `modules/actor-core` 配下にある
- **AND** trait 定義は `no_std` 対応である
- **AND** core 層から std / tokio 型への直接依存は存在しない

#### Scenario: InlineExecutor は core 層に置かれる
- **WHEN** `InlineExecutor` の定義ファイルを確認する
- **THEN** `InlineExecutor` は `modules/actor-core` 配下にある
- **AND** `InlineExecutor::execute` は現スレッドで同期に task を実行する
- **AND** `supports_blocking()` impl は持たない (trait method ごと撤去)

#### Scenario: TokioExecutor / ThreadedExecutor / PinnedExecutor は std 層に置かれる
- **WHEN** 各 std 側 executor 具象の定義ファイルを確認する
- **THEN** これらは `modules/actor-adaptor-std` 配下にある
- **AND** すべて `Executor` trait を `&mut self` command / `&self` query の契約で実装する
- **AND** `TokioExecutor` は `tokio-executor` feature 下でのみ提供される
- **AND** いずれも `supports_blocking()` impl は持たない (trait method ごと撤去)
