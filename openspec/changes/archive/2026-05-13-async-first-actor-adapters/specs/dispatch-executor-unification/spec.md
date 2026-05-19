## ADDED Requirements

### Requirement: Tokio task executor は既存 TokioExecutor を壊さず追加される

`actor-adaptor-std` は Tokio 環境向け async-first default dispatcher 用 executor として、追加 API の `TokioTaskExecutor` と `TokioTaskExecutorFactory` を提供しなければならない (MUST)。`TokioTaskExecutor` は `tokio::spawn` 相当の task 実行で mailbox drain closure を起動し、`spawn_blocking` を使ってはならない (MUST NOT)。

既存 `TokioExecutor` と `TokioExecutorFactory` は source / behavior compatible に維持されなければならない (MUST)。この change は既存 `TokioExecutor` を task executor に再定義してはならず (MUST NOT)、既存 caller に `TokioTaskExecutor` への移行を要求してはならない (MUST NOT)。

`TokioTaskExecutor` は core の `Executor` trait を実装し、submit 失敗を `ExecuteError` として返さなければならない (MUST)。`TokioTaskExecutorFactory` は `TokioTaskExecutor` を生成しなければならない (MUST)。

#### Scenario: TokioTaskExecutor は `spawn_blocking` を使わない

- **GIVEN** `TokioTaskExecutor` が `Executor::execute` を呼ばれる
- **WHEN** mailbox drain closure が submit される
- **THEN** closure は Tokio task として起動される
- **AND** `TokioTaskExecutor` の実装は `spawn_blocking` を呼ばない

#### Scenario: 既存 TokioExecutor は互換 executor として維持される

- **GIVEN** 既存 caller が `TokioExecutor::new(handle)` または `TokioExecutorFactory::new(handle)` を使っている
- **WHEN** 本 change が適用された後に同じ code を compile / run する
- **THEN** caller の source code は変更不要である
- **AND** `TokioExecutor` は `spawn_blocking` 互換の executor として維持される

#### Scenario: Tokio task executor factory は task executor を生成する

- **WHEN** `TokioTaskExecutorFactory::create(id)` が呼ばれる
- **THEN** 返却される `ExecutorShared` は `TokioTaskExecutor` を保持する
- **AND** 既存 `TokioExecutorFactory::create(id)` の返却型や意味論は変更されない

### Requirement: opt-in std Tokio 構成は default dispatcher と blocking dispatcher に別 executor を登録する

std Tokio 用の opt-in actor system 構築 helper は、`actor-core-kernel` の `DEFAULT_DISPATCHER_ID` と `DEFAULT_BLOCKING_DISPATCHER_ID` に別々の dispatcher configurator を登録しなければならない (MUST)。default dispatcher は `TokioTaskExecutorFactory` を使い、blocking dispatcher は既存 `TokioExecutorFactory` または additive な `TokioBlockingExecutorFactory` を使わなければならない (MUST)。

`ActorSystemConfig::default()` の core 単体既定は inline executor のまま維持してよい (MAY)。既存 `ActorSystemConfig::new(...)` や既存 dispatcher factory の呼び出し方を変更してはならない (MUST NOT)。ただし opt-in std Tokio helper を使った場合、blocking dispatcher が default dispatcher と同じ executor を共有してはならない (MUST NOT)。

#### Scenario: opt-in std Tokio helper は default dispatcher に task executor を登録する

- **WHEN** std Tokio helper で `ActorSystemConfig` を構築する
- **THEN** `DEFAULT_DISPATCHER_ID` には `TokioTaskExecutorFactory` ベースの dispatcher configurator が登録される
- **AND** default dispatcher で実行される mailbox drain は Tokio task executor に submit される

#### Scenario: opt-in std Tokio helper は blocking dispatcher に blocking executor を登録する

- **WHEN** std Tokio helper で `ActorSystemConfig` を構築する
- **THEN** `DEFAULT_BLOCKING_DISPATCHER_ID` には `spawn_blocking` 互換 executor factory ベースの dispatcher configurator が登録される
- **AND** `DispatcherSelector::Blocking` を指定した actor は blocking executor 側で mailbox drain される

#### Scenario: core default は no_std 依存を増やさない

- **WHEN** `ActorSystemConfig::default()` を `actor-core-kernel` 単体で構築する
- **THEN** Tokio 型への依存は発生しない
- **AND** inline executor backed default dispatcher は維持される

### Requirement: std showcase は async-first adapter の利用例を提供する

`showcases/std` は、std Tokio 用 opt-in helper、default Tokio task executor、blocking dispatcher、typed `pipe_to_self` を組み合わせた実行可能サンプルを提供しなければならない (MUST)。サンプルは `showcases/std/typed/async-first-actor-adapters/main.rs` に置かれ、`showcases/std/Cargo.toml` の `[[example]]` に `typed_async_first_actor_adapters` として登録されなければならない (MUST)。

サンプルは `modules/**/examples` には置いてはならない (MUST NOT)。Tokio 依存を使う場合は、既存 showcase crate の feature 方針に従い `advanced` feature 付き example として実行できなければならない (MUST)。

#### Scenario: showcase は std Tokio helper の dispatcher 分離を示す

- **WHEN** `cargo run -p fraktor-showcases-std --features advanced --example typed_async_first_actor_adapters` を実行する
- **THEN** actor system は std Tokio 用 opt-in helper で構築される
- **AND** default dispatcher は `TokioTaskExecutorFactory` ベースで構成される
- **AND** blocking workload は `DispatcherSelector::Blocking` を指定した actor に分離される

#### Scenario: showcase は future completion を typed self message に戻す

- **GIVEN** showcase の typed actor が async computation を開始する
- **WHEN** computation が完了する
- **THEN** actor は `pipe_to_self` 経由で completion message を受け取る
- **AND** actor state の更新は completion message handler 内で同期的に行われる

### Requirement: mailbox drain は async-first adapter 下でも non-awaiting 境界でなければならない

Tokio / Embassy などの async execution adapter を使う場合でも、`Mailbox::run`、`MessageDispatcherShared::register_for_execution`、`MessageInvoker::invoke`、`Actor::receive` の core contract は sync / non-awaiting のままでなければならない (MUST)。async execution adapter は mailbox drain closure を実行する外側の task / signal / queue を提供し、mailbox drain loop の内部に `.await` を持ち込んではならない (MUST NOT)。

#### Scenario: Tokio task executor は mailbox drain closure を同期的に実行する

- **GIVEN** `TokioTaskExecutor` が mailbox drain closure を Tokio task として起動している
- **WHEN** task が poll される
- **THEN** task 内では `Mailbox::run(throughput, throughput_deadline)` が同期的に呼ばれる
- **AND** `Mailbox::run` の内部で `.await` は発生しない

#### Scenario: async adapter は core trait を future submit に変更しない

- **WHEN** `Executor` trait のシグネチャを確認する
- **THEN** `execute` は `Box<dyn FnOnce() + Send + 'static>` を受け取る sync submit primitive のままである
- **AND** `Executor` trait に実行環境固有の `Future` 型パラメータは追加されない
