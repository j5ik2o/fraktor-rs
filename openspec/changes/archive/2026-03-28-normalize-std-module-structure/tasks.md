## 1. モジュールファイルの作成

- [x] 1.1 `std/dispatch.rs` を作成（`pub mod dispatcher;`）
- [x] 1.2 `std/dispatch/dispatcher.rs` を作成（dispatch_executor, dispatcher_config, pinned_dispatcher, schedule_adapter の宣言 + pub use）
- [x] 1.3 `std/event.rs` を作成（`pub mod logging; pub mod stream;`）
- [x] 1.4 `std/event/logging.rs` を作成（tracing_logger_subscriber の宣言 + pub use）
- [x] 1.5 `std/event/stream.rs` を作成（dead_letter_log_subscriber, subscriber, subscriber_adapter の宣言 + pub use）
- [x] 1.6 `std/scheduler.rs` を作成（tick の cfg 付き宣言）
- [x] 1.7 `std/system.rs` を作成（base, coordinated_shutdown 等の宣言 + pub use）
- [x] 1.8 `std/typed.rs` を作成（behaviors, log_options の宣言 + pub use）

## 2. std.rs の正規化

- [x] 2.1 `std.rs` のインラインモジュール定義を `pub mod xxx;` 外部参照に置き換える

## 3. 検証

- [x] 3.1 `cargo check -p fraktor-actor-rs --features tokio-executor` パス
- [x] 3.2 `cargo test -p fraktor-actor-rs --features tokio-executor --lib` 1136テスト全パス
- [x] 3.3 dylint lint パス
