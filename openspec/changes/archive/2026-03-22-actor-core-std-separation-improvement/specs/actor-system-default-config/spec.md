## ADDED Requirements

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

## REMOVED Requirements

### Requirement: ActorSystem::quickstart()
**Reason**: Pekko にも protoactor にもない命名。`new()` のデフォルト構成で代替。
**Migration**: `ActorSystem::quickstart(&props)` → `ActorSystem::new(&props)`

### Requirement: ActorSystem::quickstart_with()
**Reason**: `quickstart` 廃止に伴い削除。カスタマイズは `new_with_config()` で対応。
**Migration**: `ActorSystem::quickstart_with(&props, configure)` → `ActorSystem::new_with_config(&props, &config)`（configure クロージャで設定を構築してから渡す）

### Requirement: TickDriverConfig::tokio_quickstart()
**Reason**: `Default` trait 実装で代替。
**Migration**: `TickDriverConfig::tokio_quickstart()` → `TickDriverConfig::default()`

### Requirement: TickDriverConfig::tokio_quickstart_with_resolution()
**Reason**: 命名改善。resolution 指定は別メソッドで提供。
**Migration**: `TickDriverConfig::tokio_quickstart_with_resolution(dur)` → `TickDriverConfig::with_resolution(dur)`（新設メソッド）

### Requirement: DispatcherConfig::tokio_auto()
**Reason**: `Default` trait 実装で代替。
**Migration**: `DispatcherConfig::tokio_auto()` → `DispatcherConfig::default()`
