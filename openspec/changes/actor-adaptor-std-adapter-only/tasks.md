## 1. Core Logging Module Wiring

- [ ] 1.1 `core::kernel::event::logging` に classic logging family の配置先を追加する
- [ ] 1.2 `core::kernel::event::logging` の module wiring と re-export を更新する

## 2. Existing Type Relocation

Section 1 の module wiring 完了後に着手する。

- [ ] 2.1 `ActorLogMarker` を `std/event/logging` から `core` へ移設する
- [ ] 2.2 `LoggingAdapter` を `std/event/logging` から `core` へ移設する
- [ ] 2.3 `ActorLogging` を `std/event/logging` から `core` へ移設する
- [ ] 2.4 `DiagnosticActorLogging` を `std/event/logging` から `core` へ移設する
- [ ] 2.5 `BusLogging` を `std/event/logging` から `core` へ移設する
- [ ] 2.6 `LoggingReceive` を `std/event/logging` から `core` へ移設する
- [ ] 2.7 `NoLogging` を `std/event/logging` から `core` へ移設する

## 3. Std Logging Surface Reduction

- [ ] 3.1 `showcases/std/classic_logging/main.rs` を `core::kernel::event::logging` の import path に更新する
- [ ] 3.2 event stream subscriber を使う showcase と `modules/actor-adaptor/src/std/tests.rs` を `core` subscriber 型へ移行する
- [ ] 3.3 `modules/actor-adaptor/src/std/tests.rs` の公開面確認を新しい `std` surface に合わせて更新する

## 4. Shim And Wrapper Removal

- [ ] 4.1 `std/system/*` の subscriber 型参照を `core::kernel::event::stream` へ切り替える
- [ ] 4.2 `modules/actor-adaptor/src/std/event/logging` の公開面を `TracingLoggerSubscriber` のみに縮小する
- [ ] 4.3 `modules/actor-adaptor/src/std/event/stream` から `EventStreamSubscriberShared` と `subscriber_handle` の shim を削除する
- [ ] 4.4 `modules/actor-adaptor/src/std/pattern::{ask_with_timeout, graceful_stop, graceful_stop_with_message, retry}` の利用箇所を棚卸しし、削除に伴う更新対象を確定する
- [ ] 4.5 `modules/actor-adaptor/src/std/pattern` から `ask_with_timeout`、`graceful_stop`、`graceful_stop_with_message`、`retry` を削除する
- [ ] 4.6 package boundary test を更新し、`std::event::stream` は `DeadLetterLogSubscriber` のみ、`std::pattern` は `StdClock` / circuit breaker 系のみが残ることを固定する

## 5. Showcase And Verification

- [ ] 5.1 移設後も marker / MDC / receive logging / facade / no-op logging の既存挙動が維持されるように tests を更新する
- [ ] 5.2 `./scripts/ci-check.sh examples`、`./scripts/ci-check.sh std`、`./scripts/ci-check.sh ai all` で最終確認する
