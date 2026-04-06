# actor-classic-logging-core Specification

## Purpose
classic logging family を `core` の no_std public surface として提供し、`std` adapter への不要な依存を排除する。

## Requirements
### Requirement: classic logging family は `core` の no_std public surface として提供される
actor runtime は、既存 classic logging family を `core::kernel::event::logging` の public surface として提供しなければならない。`ActorLogMarker`、`ActorLogging`、`BusLogging`、`DiagnosticActorLogging`、`LoggingAdapter`、`LoggingReceive`、`NoLogging` は `std` backend に依存せず、no_std / Sans I/O の `core` から利用可能であることを MUST 満たす。

#### Scenario: classic logging family の import path が `core` に存在する
- **WHEN** 利用者が `fraktor_actor_core_rs::core::kernel::event::logging` から `ActorLogMarker`、`ActorLogging`、`BusLogging`、`DiagnosticActorLogging`、`LoggingAdapter`、`LoggingReceive`、`NoLogging` を import する
- **THEN** その import はコンパイルできる
- **AND** `fraktor_actor_adaptor_rs::std::event::logging` を経由する必要はない

### Requirement: classic logging family の既存挙動は移設後も維持される
classic logging family の既存挙動は、`std` から `core` へ移設した後も維持されなければならない。`ActorLogMarker`、`ActorLogging`、`BusLogging`、`DiagnosticActorLogging`、`LoggingAdapter`、`LoggingReceive`、`NoLogging` の既存 tests で確認している marker / MDC / receive logging / facade / no-op logging の挙動は MUST 変化しない。

#### Scenario: actor logging facade は移設後も同じ adapter を返す
- **WHEN** 利用者が `core::kernel::event::logging::ActorLogging` を actor context から生成して `log()` を呼ぶ
- **THEN** `log()` は `LoggingAdapter` を返す
- **AND** その adapter 経由で emit した log event は既存 `std` 実装と同じ内容を観測できる

#### Scenario: bus logging facade は移設後も system 経由で log event を emit する
- **WHEN** 利用者が `core::kernel::event::logging::BusLogging` を使って log を emit する
- **THEN** publish される `LogEvent` は既存 `std` 実装と同じ level、message、origin、logger_name を保持する

#### Scenario: marker と MDC を含む log event が移設後も生成される
- **WHEN** 利用者が `core::kernel::event::logging::LoggingAdapter` と `ActorLogMarker` を使って marker と MDC を含む log を emit する
- **THEN** publish される `LogEvent` は marker 名、marker properties、MDC を保持する
- **AND** 既存 `std` 実装と同じ内容を観測できる

#### Scenario: receive logging が移設後も生成される
- **WHEN** 利用者が `core::kernel::event::logging::LoggingReceive` を使って handled または unhandled message を記録する
- **THEN** publish される `LogEvent` の message には既存 `std` 実装と同じ receive logging 文言が含まれる

#### Scenario: no logging は移設後も no-op を維持する
- **WHEN** 利用者が `core::kernel::event::logging::NoLogging` に対して trace、debug、info、warn、error を呼ぶ
- **THEN** log event は publish されない
- **AND** 呼び出しは既存 `std` 実装と同様に安全に完了する
