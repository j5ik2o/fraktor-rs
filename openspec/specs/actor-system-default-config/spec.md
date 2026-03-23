# actor-system-default-config Specification

## Purpose
TBD - created by archiving change actor-core-std-separation-improvement. Update Purpose after archive.
## Requirements
### Requirement: ActorSystem::new() が feature gate に応じたデフォルト構成を提供する

`ActorSystem::new(&props)` が `tokio-executor` feature 有効時に `TickDriverConfig::default()` と `DispatcherConfig::default()` を使って自動構成する。ユーザーは明示的な設定なしで ActorSystem を起動できる。

#### Scenario: tokio-executor 有効時に new() がデフォルトで動作する
- **WHEN** `#[cfg(feature = "tokio-executor")]` が有効な環境で `ActorSystem::new(&props)` を呼び出す
- **THEN** 10ms resolution の TickDriver と現在の Tokio runtime handle を使った Dispatcher でシステムが起動する

#### Scenario: カスタム設定が必要な場合は new_with_config を使用する
- **WHEN** デフォルトと異なる TickDriver resolution や Dispatcher を指定したい
- **THEN** `ActorSystem::new_with_config(&props, &config)` で任意の `ActorSystemConfig` を渡せる

### Requirement: TickDriverConfig::default() がデフォルト構成を返す

`TickDriverConfig` に `Default` trait を実装する。`tokio-executor` feature 有効時は 10ms resolution の Tokio ベース TickDriver 構成を返す。

#### Scenario: tokio-executor 有効時のデフォルト
- **WHEN** `TickDriverConfig::default()` を呼び出す
- **THEN** 10ms resolution の Tokio TickDriver 構成が返される（現在の `tokio_quickstart()` と同等）

### Requirement: DispatcherConfig::default() がデフォルト構成を返す

`DispatcherConfig` に `Default` trait を実装する。`tokio-executor` feature 有効時は現在の Tokio runtime handle を自動検出して Dispatcher を構成する。

#### Scenario: tokio-executor 有効時のデフォルト
- **WHEN** Tokio runtime 内で `DispatcherConfig::default()` を呼び出す
- **THEN** 現在の runtime handle を使った TokioExecutor ベースの構成が返される（現在の `tokio_auto()` と同等）

#### Scenario: Tokio runtime 外で呼び出した場合のエラー
- **WHEN** Tokio runtime 外で `DispatcherConfig::default()` を呼び出す
- **THEN** panic する（現在の `tokio_auto()` と同じ挙動）

