# Implementation Tasks

## 実行前の注意
- Dylint 系のリント（`module-wiring-lint` や `mod-file-lint` 等）を前提とした構成でタスクが組まれているため、各フェーズでファイル移動や可視性変更を行ったらすぐに `makers ci-check` もしくは該当リント単体のテストを実行し、早期に違反を検知すること。
- 特に大規模なファイル移動を行う前に、移動先ディレクトリやモジュール構成がリント要件（`mod.rs` 禁止、1ファイル1構造体/1trait など）を満たしているかを確認してから作業を進めること。

## フェーズ1: パッケージレイアウトの整備

Rust 2018 のモジュール規約に従い、各論理パッケージは「ルートファイル + ディレクトリ」の組み合わせで構成する。`mod.rs` は使用しない。

### 1. ルートファイル／ディレクトリの作成
- [ ] `actor_prim.rs` + `actor_prim/`
- [ ] `messaging.rs` + `messaging/`
- [ ] `mailbox.rs` + `mailbox/`
- [ ] `supervision.rs` + `supervision/`
- [ ] `props.rs` + `props/`
- [ ] `spawn.rs` + `spawn/`
- [ ] `system.rs` + `system/`
- [ ] `eventstream.rs` + `eventstream/`
- [ ] `lifecycle.rs` + `lifecycle/`
- [ ] `deadletter.rs` + `deadletter/`
- [ ] `logging.rs` + `logging/`
- [ ] `futures.rs` + `futures/`
- [ ] `error.rs` + `error/`

> ルートファイルは子モジュールを `pub mod ...;` で公開するのみとし、`pub use` による再エクスポートは禁止する。

### 2. ファイル移動とリネーム
- [ ] `actor.rs` → `actor_prim/actor.rs`
- [ ] `actor_ref.rs` → `actor_prim/actor_ref.rs`
- [ ] `actor_ref_impl.rs` → `actor_prim/actor_ref_internal.rs`
- [ ] `actor_cell.rs` → `actor_prim/actor_cell.rs`
- [ ] `actor_context.rs` → `actor_prim/actor_context.rs`
- [ ] `pid.rs` → `actor_prim/pid.rs`
- [ ] `child_ref.rs` → `actor_prim/child_ref.rs`
- [ ] `receive_state.rs` → `actor_prim/receive_state.rs`
- [ ] `any_message*.rs` → `messaging/`
- [ ] `message_invoker*.rs` → `messaging/`
- [ ] `system_message.rs` → `messaging/system_message.rs`
- [ ] `mailbox*.rs` を `mailbox/` へ移動（`mailbox_capacity.rs` は `mailbox/capacity.rs` に改名）
- [ ] `supervisor_strategy.rs` → `supervision/strategy.rs`
- [ ] `restart_statistics.rs` → `supervision/restart_statistics.rs`
- [ ] `props_struct.rs` → `props/props.rs`
- [ ] `props_actor_factory.rs` → `props/factory.rs`
- [ ] `props_mailbox_config.rs` → `props/mailbox_config.rs`
- [ ] `props_dispatcher_config.rs` → `props/dispatcher_config.rs`
- [ ] `props_supervisor_options.rs` → `props/supervisor_options.rs`
- [ ] `spawn_error.rs` → `spawn/spawn_error.rs`
- [ ] `name_registry.rs` → `spawn/name_registry.rs`
- [ ] `name_registry_error.rs` → `spawn/name_registry_error.rs`
- [ ] `system.rs` → `system/root.rs`（`system.rs` では `pub mod root;` のみ宣言）
- [ ] `system_state.rs` → `system/system_state.rs`
- [ ] `dispatcher.rs` → `system/dispatcher.rs`
- [ ] `event_stream*.rs` → `eventstream/`
- [ ] `lifecycle_event.rs`, `lifecycle_stage.rs` → `lifecycle/`
- [ ] `deadletter*.rs` → `deadletter/`
- [ ] `log_event.rs`, `log_level.rs`, `logger_*` → `logging/`
- [ ] `actor_future*.rs` → `futures/`
- [ ] `actor_error*.rs`, `send_error.rs` → `error/`
- [ ] 関連するテスト・ベンチファイルも移動（`tests/`、`benches/`）

### 3. 内部実装の可視性調整
- [ ] `actor_ref_internal.rs` など内部ファイルに `pub(crate)` を適用
- [ ] ルートファイル（例: `actor_prim.rs`）は `pub mod` 宣言のみとし、`pub use` を追加しない
- [ ] 内部ヘルパを参照している箇所を新しい階層パスへ更新

### 4. `lib.rs` の更新
- [ ] 新しいモジュール宣言 (`pub mod actor_prim;` など) を追加
- [ ] 旧来のフラットな再エクスポートを削除し、新しい階層パスのみを公開
- [ ] `prelude` から必要な再エクスポートを提供し、内部からの FQCN 利用へ統一

## フェーズ2: ビルド・テスト
- [ ] `cargo fmt`
- [ ] `cargo clippy -- -D warnings`
- [ ] `cargo test`（`--no-default-features` / `--features std`）
- [ ] `cargo doc --no-deps`
- [ ] `./scripts/ci-check.sh all`

## フェーズ3: ドキュメント整備
- [ ] `docs/guides/module_wiring.md` に新構造の説明を追記
- [ ] 新しいパッケージ構造に沿った API ドキュメントコメントを各ルートファイルに追加
- [ ] 利用者向け `prelude` のリストを更新
- [ ] 旧→新インポートパス対応表を作成（マイグレーションガイド）

## フェーズ4: OpenSpec 検証
- [ ] `openspec validate refactor-actor-core-package-structure --strict`
- [ ] レビューで指摘された項目を反映して完了
