# Implementation Tasks

## フェーズ1: ディレクトリ構造の作成とファイル移動

### パッケージディレクトリの作成
- [ ] `actor/` ディレクトリとサブディレクトリ作成
  - [ ] `actor/internal/` ディレクトリ作成
- [ ] `messaging/` ディレクトリ作成
- [ ] `mailbox/` ディレクトリ作成
- [ ] `supervision/` ディレクトリ作成
- [ ] `props/` ディレクトリ作成
- [ ] `spawn/` ディレクトリ作成
- [ ] `system/` ディレクトリ作成
- [ ] `eventstream/` ディレクトリ作成
- [ ] `lifecycle/` ディレクトリ作成
- [ ] `deadletter/` ディレクトリ作成
- [ ] `logging/` ディレクトリ作成
- [ ] `futures/` ディレクトリ作成
- [ ] `error/` ディレクトリ作成

### ファイル移動: actor/ パッケージ
- [ ] `actor.rs` → `actor/actor.rs`
- [ ] `actor_ref.rs` → `actor/actor_ref.rs`
- [ ] `actor_cell.rs` → `actor/actor_cell.rs`
- [ ] `actor_context.rs` → `actor/actor_context.rs`
- [ ] `pid.rs` → `actor/pid.rs`
- [ ] `child_ref.rs` → `actor/child_ref.rs`
- [ ] `receive_state.rs` → `actor/receive_state.rs`
- [ ] `actor_ref_impl.rs` → `actor/internal/actor_ref_impl.rs`（内部実装）
- [ ] `actor_ref/` サブディレクトリ内の実装ファイルを `actor/internal/` へ移動
- [ ] 対応するテストファイルを移動

### ファイル移動: messaging/ パッケージ
- [ ] `any_message.rs` → `messaging/any_message.rs`
- [ ] `any_message_view.rs` → `messaging/any_message_view.rs`
- [ ] `ask_response.rs` → `messaging/ask_response.rs`
- [ ] `message_invoker.rs` → `messaging/message_invoker.rs`
- [ ] `system_message.rs` → `messaging/system_message.rs`
- [ ] 対応するテストファイルを移動

### ファイル移動: mailbox/ パッケージ
- [ ] `mailbox.rs` → `mailbox/mailbox.rs`
- [ ] `mailbox_capacity.rs` → `mailbox/capacity.rs`
- [ ] `mailbox_policy.rs` → `mailbox/policy.rs`
- [ ] `mailbox_overflow_strategy.rs` → `mailbox/overflow_strategy.rs`
- [ ] `mailbox_metrics_event.rs` → `mailbox/metrics.rs`
- [ ] `mailbox/` サブディレクトリ内のファイルを整理
- [ ] 対応するテストファイルを移動

### ファイル移動: supervision/ パッケージ
- [ ] `supervisor_strategy.rs` → `supervision/strategy.rs`
- [ ] `supervisor_strategy` の関連型を `supervision/directive.rs` に分離
- [ ] `restart_statistics.rs` → `supervision/restart_statistics.rs`
- [ ] `props_supervisor_options.rs` → `supervision/options.rs`
- [ ] 対応するテストファイルを移動

### ファイル移動: props/ と spawn/ パッケージ
- [ ] `props_struct.rs` → `props/props.rs`
- [ ] `props_actor_factory.rs` → `props/factory.rs`
- [ ] `props_mailbox_config.rs` → `props/mailbox_config.rs`
- [ ] `props_dispatcher_config.rs` → `props/dispatcher_config.rs`
- [ ] `props_supervisor_options.rs` → `props/supervisor_options.rs`（supervision/からコピー）
- [ ] `spawn_error.rs` → `spawn/spawn_error.rs`
- [ ] `name_registry.rs` → `spawn/name_registry.rs`
- [ ] `name_registry_error.rs` → `spawn/name_registry_error.rs`
- [ ] 対応するテストファイルを移動

### ファイル移動: system/ パッケージ
- [ ] `system.rs` → `system/system.rs`
- [ ] `system_state.rs` → `system/system_state.rs`
- [ ] `dispatcher.rs` → `system/dispatcher.rs`
- [ ] 対応するテストファイルを移動

### ファイル移動: eventstream/ パッケージ
- [ ] `event_stream.rs` → `eventstream/event_stream.rs`
- [ ] `event_stream_event.rs` → `eventstream/event.rs`
- [ ] `event_stream_subscriber.rs` → `eventstream/subscriber.rs`
- [ ] `event_stream_subscriber_entry.rs` → `eventstream/subscriber_entry.rs`
- [ ] `event_stream_subscription.rs` → `eventstream/subscription.rs`
- [ ] 対応するテストファイルを移動

### ファイル移動: その他のパッケージ
- [ ] lifecycle/ パッケージへのファイル移動
  - [ ] `lifecycle_event.rs` → `lifecycle/event.rs`
  - [ ] `lifecycle_stage.rs` → `lifecycle/stage.rs`
- [ ] deadletter/ パッケージへのファイル移動
  - [ ] `deadletter.rs` → `deadletter/deadletter.rs`
  - [ ] `deadletter_entry.rs` → `deadletter/entry.rs`
  - [ ] `deadletter_reason.rs` → `deadletter/reason.rs`
- [ ] logging/ パッケージへのファイル移動
  - [ ] `log_event.rs` → `logging/event.rs`
  - [ ] `log_level.rs` → `logging/level.rs`
  - [ ] `logger_subscriber.rs` → `logging/subscriber.rs`
  - [ ] `logger_writer.rs` → `logging/writer.rs`
- [ ] futures/ パッケージへのファイル移動
  - [ ] `actor_future.rs` → `futures/actor_future.rs`
  - [ ] `actor_future_listener.rs` → `futures/listener.rs`
- [ ] error/ パッケージへのファイル移動
  - [ ] `actor_error.rs` → `error/actor_error.rs`
  - [ ] `actor_error_reason.rs` → `error/actor_error_reason.rs`
  - [ ] `send_error.rs` → `error/send_error.rs`
- [ ] 対応するテストファイルをすべて移動

### mod.rs ファイルの作成
- [ ] `actor/mod.rs` 作成（サブモジュール宣言と `pub use` 再エクスポート）
- [ ] `actor/internal/mod.rs` 作成（`pub(crate)` 使用）
- [ ] `messaging/mod.rs` 作成
- [ ] `mailbox/mod.rs` 作成
- [ ] `supervision/mod.rs` 作成
- [ ] `props/mod.rs` 作成
- [ ] `spawn/mod.rs` 作成
- [ ] `system/mod.rs` 作成
- [ ] `eventstream/mod.rs` 作成
- [ ] `lifecycle/mod.rs` 作成
- [ ] `deadletter/mod.rs` 作成
- [ ] `logging/mod.rs` 作成
- [ ] `futures/mod.rs` 作成
- [ ] `error/mod.rs` 作成

### lib.rs の更新
- [ ] パッケージモジュール宣言を追加
- [ ] 既存の公開APIを維持するための `pub use` 再エクスポートを追加
- [ ] 後方互換性のテスト確認

## フェーズ2: ビルドとテストの検証

### ビルド検証
- [ ] `cargo build` 実行（エラーがないことを確認）
- [ ] `cargo build --all-features` 実行
- [ ] `cargo build --no-default-features` 実行

### テスト実行
- [ ] `cargo test` 実行（全テストパス）
- [ ] `cargo test --all-features` 実行
- [ ] `cargo test --doc` 実行（ドキュメントテスト）

### 静的解析
- [ ] `cargo clippy -- -D warnings` 実行
- [ ] 社内Dylint実行（`./scripts/ci-check.sh all` または `makers ci-check`）
- [ ] `cargo fmt -- --check` 実行

### ドキュメント生成
- [ ] `cargo doc --no-deps` 実行
- [ ] 生成されたドキュメントの構造確認
- [ ] リンク切れがないことを確認

## フェーズ3: ドキュメントとPreludeの整備

### パッケージドキュメントの追加
- [ ] `actor/mod.rs` にモジュールレベルドキュメント追加
- [ ] `messaging/mod.rs` にモジュールレベルドキュメント追加
- [ ] `mailbox/mod.rs` にモジュールレベルドキュメント追加
- [ ] `supervision/mod.rs` にモジュールレベルドキュメント追加
- [ ] `props/mod.rs` にモジュールレベルドキュメント追加
- [ ] `spawn/mod.rs` にモジュールレベルドキュメント追加
- [ ] `system/mod.rs` にモジュールレベルドキュメント追加
- [ ] `eventstream/mod.rs` にモジュールレベルドキュメント追加
- [ ] その他のパッケージのドキュメント追加

### 使用例の追加
- [ ] 主要なパッケージに使用例を追加
- [ ] `prelude` の使用例を追加

### prelude.rs の作成
- [ ] `prelude.rs` ファイル作成
- [ ] よく使う型を集約（Actor, ActorRef, ActorContext, Props, ActorSystem等）
- [ ] ドキュメント追加

### マイグレーションガイド
- [ ] `claudedocs/` または適切な場所にマイグレーションガイド作成
- [ ] 新旧インポートパスの対応表を作成
- [ ] 推奨される移行方法を記載

## 最終検証
- [ ] すべてのCI チェックをパス
- [ ] ドキュメントの完全性確認
- [ ] 後方互換性の最終確認
- [ ] OpenSpec validation 実行（`openspec validate refactor-actor-core-package-structure --strict`）
