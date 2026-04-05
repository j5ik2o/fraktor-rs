## Why

この change は `modules/actor` と `modules/actor-adaptor` の 2 クレートを横断する。

`modules/actor-adaptor/src/std` には、`core` が担うべき classic logging の public surface と、`core` の型を横流しするだけの shim / wrapper が混在している。これにより `std` が adapter-only という境界を崩し、Pekko 互換をうたいながら logging の意味論が runtime adapter 側へ漏れている。

いま整理する理由は二つある。ひとつは `core` を no_std / Sans I/O のまま既存 classic logging family を持てる形に戻すため、もうひとつは `std` 公開面を `tracing` / `tokio` / `std` 依存の adapter 実装だけへ収束させるためである。

## What Changes

- `ActorLogMarker`、`ActorLogging`、`BusLogging`、`DiagnosticActorLogging`、`LoggingAdapter`、`LoggingReceive`、`NoLogging` を `core` 側の classic logging public surface として再配置する
- `modules/actor-adaptor/src/std/event/logging` は `TracingLoggerSubscriber` のみを公開する adapter surface に縮小する
- **BREAKING** `modules/actor-adaptor/src/std/event/stream` の shim API を削除し、利用側を `core::kernel::event::stream` の型へ統一する
- **BREAKING** `modules/actor-adaptor/src/std/pattern` の core 横流し helper を削除し、必要な std 固有要素は `StdClock` ベースの circuit breaker 系に限定する
- showcase / tests / package boundary test を、新しい `core` import path と adapter-only な `std` 公開面に追随させる
- message-scoped MDC、`LoggingReceive` の設定連動、timestamp 一元化の API 強化、Pekko にある未実装型の追加は follow-up change として分離する

## Capabilities

### New Capabilities
- `actor-classic-logging-core`: 既存 classic logging family を no_std / Sans I/O の `core` public surface として提供する

### Modified Capabilities
- `actor-std-adapter-surface`: `std` 公開面から classic logging facade、event stream shim、core 横流し helper を除去し、adapter-only 境界へ更新する

## Impact

- 影響コード:
  - `modules/actor/src/core/kernel/event/logging.rs`
  - `modules/actor/src/core/kernel/event/logging/*`
  - `modules/actor-adaptor/src/std/event/logging/*`
  - `modules/actor-adaptor/src/std/event/stream.rs`
  - `modules/actor-adaptor/src/std/pattern.rs`
  - `modules/actor-adaptor/src/std/pattern/tests.rs`
  - `showcases/std/*`
- 影響 API:
  - `fraktor_actor_rs::core::kernel::event::logging` に classic logging family が追加される
  - `fraktor_actor_adaptor_rs::std` から logging facade / shim / wrapper の一部が削除される
  - 現時点で `std::pattern::{ask_with_timeout, graceful_stop, graceful_stop_with_message, retry}` の利用箇所は `modules/actor-adaptor/src/std/pattern/tests.rs` のみである
- 互換性:
  - 後方互換は不要
  - ただしこの change は配置整理を主目的とし、意味論の再設計は含めない
- 依存:
  - `core` は no_std / Sans I/O を維持する
  - `std` は `tracing` などの backend adapter 実装のみを保持する
