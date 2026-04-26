## ADDED Requirements

### Requirement: Tokio executor family は default task executor と blocking executor を分離する

`actor-adaptor-std` は Tokio 環境向け executor を default dispatcher 用と blocking dispatcher 用に分離しなければならない (MUST)。default dispatcher 用の `TokioExecutor` は `tokio::spawn` 相当の task 実行で mailbox drain closure を起動し、`spawn_blocking` を使ってはならない (MUST NOT)。blocking dispatcher 用の `TokioBlockingExecutor` は `tokio::task::spawn_blocking` で task を起動しなければならない (MUST)。

両 executor は core の `Executor` trait を実装し、submit 失敗を `ExecuteError` として返さなければならない (MUST)。`TokioExecutorFactory` は `TokioExecutor` を生成し、`TokioBlockingExecutorFactory` は `TokioBlockingExecutor` を生成しなければならない (MUST)。

#### Scenario: default Tokio executor は `spawn_blocking` を使わない

- **GIVEN** `TokioExecutor` が `Executor::execute` を呼ばれる
- **WHEN** mailbox drain closure が submit される
- **THEN** closure は Tokio task として起動される
- **AND** `TokioExecutor` の実装は `spawn_blocking` を呼ばない

#### Scenario: blocking Tokio executor は `spawn_blocking` を使う

- **GIVEN** `TokioBlockingExecutor` が `Executor::execute` を呼ばれる
- **WHEN** blocking workload 用 task が submit される
- **THEN** task は `tokio::task::spawn_blocking` で起動される
- **AND** submit が拒否された場合は `ExecuteError` が返る

#### Scenario: Tokio executor factories は責務別 executor を生成する

- **WHEN** `TokioExecutorFactory::create(id)` が呼ばれる
- **THEN** 返却される `ExecutorShared` は `TokioExecutor` を保持する
- **WHEN** `TokioBlockingExecutorFactory::create(id)` が呼ばれる
- **THEN** 返却される `ExecutorShared` は `TokioBlockingExecutor` を保持する

### Requirement: std Tokio 構成は default dispatcher と blocking dispatcher に別 executor を登録する

std Tokio 用の actor system 構築 helper は、`pekko.actor.default-dispatcher` と `pekko.actor.default-blocking-io-dispatcher` に別々の dispatcher configurator を登録しなければならない (MUST)。default dispatcher は `TokioExecutorFactory` を使い、blocking dispatcher は `TokioBlockingExecutorFactory` を使わなければならない (MUST)。

`ActorSystemConfig::default()` の core 単体既定は inline executor のまま維持してよい (MAY)。ただし std Tokio helper を使った場合、blocking dispatcher が default dispatcher と同じ executor を共有してはならない (MUST NOT)。

#### Scenario: std Tokio helper は default dispatcher に task executor を登録する

- **WHEN** std Tokio helper で `ActorSystemConfig` を構築する
- **THEN** `pekko.actor.default-dispatcher` には `TokioExecutorFactory` ベースの dispatcher configurator が登録される
- **AND** default dispatcher で実行される mailbox drain は Tokio task executor に submit される

#### Scenario: std Tokio helper は blocking dispatcher に blocking executor を登録する

- **WHEN** std Tokio helper で `ActorSystemConfig` を構築する
- **THEN** `pekko.actor.default-blocking-io-dispatcher` には `TokioBlockingExecutorFactory` ベースの dispatcher configurator が登録される
- **AND** `DispatcherSelector::Blocking` を指定した actor は blocking executor 側で mailbox drain される

#### Scenario: core default は no_std 依存を増やさない

- **WHEN** `ActorSystemConfig::default()` を actor-core 単体で構築する
- **THEN** Tokio 型への依存は発生しない
- **AND** inline executor backed default dispatcher は維持される

### Requirement: mailbox drain は async-first adapter 下でも non-awaiting 境界でなければならない

Tokio / Embassy などの async runtime adapter を使う場合でも、`Mailbox::run`、`MessageDispatcherShared::register_for_execution`、`MessageInvoker::invoke`、`Actor::receive` の core contract は sync / non-awaiting のままでなければならない (MUST)。async runtime adapter は mailbox drain closure を実行する外側の task / signal / queue を提供し、mailbox drain loop の内部に `.await` を持ち込んではならない (MUST NOT)。

#### Scenario: Tokio task executor は mailbox drain closure を同期的に実行する

- **GIVEN** `TokioExecutor` が mailbox drain closure を Tokio task として起動している
- **WHEN** task が poll される
- **THEN** task 内では `Mailbox::run(throughput, throughput_deadline)` が同期的に呼ばれる
- **AND** `Mailbox::run` の内部で `.await` は発生しない

#### Scenario: async adapter は core trait を future submit に変更しない

- **WHEN** `Executor` trait のシグネチャを確認する
- **THEN** `execute` は `Box<dyn FnOnce() + Send + 'static>` を受け取る sync submit primitive のままである
- **AND** `Executor` trait に runtime 固有の `Future` 型パラメータは追加されない
