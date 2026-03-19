## ADDED Requirements

### Requirement: CoordinatedShutdown 型定義を core/system/ に配置する

以下の型を `std/system/` から `core/system/` に移設する。これらはプラットフォーム依存がなく、no_std 環境でも型レベルで参照可能にすべきである:

- `CoordinatedShutdownError`
- `CoordinatedShutdownId`
- `CoordinatedShutdownPhase`
- `CoordinatedShutdownReason`

移設後、`#[cfg(feature = "tokio-executor")]` gate を外す。

#### Scenario: 型定義が no_std 環境でコンパイルできる
- **WHEN** `#![no_std]` 環境で `core::system::CoordinatedShutdownId` を参照する
- **THEN** コンパイルが成功する

#### Scenario: 型定義が feature gate なしで利用できる
- **WHEN** `tokio-executor` feature が無効な環境でビルドする
- **THEN** `core::system::CoordinatedShutdownId`, `CoordinatedShutdownPhase`, `CoordinatedShutdownReason`, `CoordinatedShutdownError` がコンパイル可能である

### Requirement: std/system/ から core 型を re-export する

`std::system` モジュールで移設した型を re-export し、既存の `use crate::std::system::CoordinatedShutdownId` などの import パスを維持する。

#### Scenario: 既存の std パスでの import が維持される
- **WHEN** `use crate::std::system::CoordinatedShutdownId` で import する
- **THEN** re-export 経由でコンパイルが通る

### Requirement: CoordinatedShutdown 実行ロジックは std/ に残す

`CoordinatedShutdown` 構造体本体（`coordinated_shutdown.rs`）と `CoordinatedShutdownInstaller` は `tokio::spawn` / `tokio::time::timeout` に依存するため、`std/system/` に残す。`#[cfg(feature = "tokio-executor")]` gate を維持する。

#### Scenario: CoordinatedShutdown 実行ロジックが tokio-executor feature gate 下にある
- **WHEN** `tokio-executor` feature が無効な環境でビルドする
- **THEN** `std::system::CoordinatedShutdown` 構造体はコンパイル対象外である
