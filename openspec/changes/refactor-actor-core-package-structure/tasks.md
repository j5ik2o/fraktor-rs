# Implementation Tasks

## 実行前の注意
- Dylint 系のリント（`module-wiring-lint` や `mod-file-lint` 等）を前提とした構成でタスクが組まれているため、各フェーズでファイル移動や可視性変更を行ったらすぐに `makers ci-check` もしくは該当リント単体のテストを実行し、早期に違反を検知すること。
- 特に大規模なファイル移動を行う前に、移動先ディレクトリやモジュール構成がリント要件（`mod.rs` 禁止、1ファイル1構造体/1trait など）を満たしているかを確認してから作業を進めること。
- allowなどの一切のリント回避設定は禁止とする
- `prelude`はこのタスク外で対応するために、完全にFQCNなインポートにすること

## フェーズ1: パッケージレイアウトの整備

Rust 2018 のモジュール規約に従い、各論理パッケージは「ルートファイル + ディレクトリ」の組み合わせで構成する。`mod.rs` は使用しない。

### 1. ルートファイル／ディレクトリの作成
- [x] `actor_prim.rs` + `actor_prim/`
- [x] `messaging.rs` + `messaging/`
- [x] `mailbox.rs` + `mailbox/`
- [x] `supervision.rs` + `supervision/`
- [x] `props.rs` + `props/`
- [x] `spawn.rs` + `spawn/`
- [x] `system.rs` + `system/`
- [x] `eventstream.rs` + `eventstream/`
- [x] `lifecycle.rs` + `lifecycle/`
- [x] `deadletter.rs` + `deadletter/`
- [x] `logging.rs` + `logging/`
- [x] `futures.rs` + `futures/`
- [x] `error.rs` + `error/`
- [x] `./scripts/ci-check.sh all`

> ルートファイルはModule Wiring規約に従い、子モジュールを `mod` で宣言し、`pub use` で再エクスポートする。

### 2. ファイル移動とリネーム
- [x] `actor.rs` → `actor_prim/actor.rs`
- [x] `actor_ref.rs` → `actor_prim/actor_ref.rs`
- [x] `actor_cell.rs` → `actor_prim/actor_cell.rs`
- [x] `actor_context.rs` → `actor_prim/actor_context.rs`
- [x] `pid.rs` → `actor_prim/pid.rs`
- [x] `child_ref.rs` → `actor_prim/child_ref.rs`
- [x] `receive_state.rs` → `actor_prim/receive_state.rs`
- [x] `any_message*.rs` → `messaging/`
- [x] `message_invoker*.rs` → `messaging/`
- [x] `system_message.rs` → `messaging/system_message.rs`
- [x] `mailbox*.rs` を `mailbox/` へ移動（`mailbox_capacity.rs` は `mailbox/capacity.rs` に改名）
- [x] `supervisor_strategy.rs` → `supervision/strategy.rs`
- [x] `restart_statistics.rs` → `supervision/restart_statistics.rs`
- [x] `props_struct.rs` → `props/props.rs`
- [x] `props_actor_factory.rs` → `props/factory.rs`
- [x] `props_mailbox_config.rs` → `props/mailbox_config.rs`
- [x] `props_dispatcher_config.rs` → `props/dispatcher_config.rs`
- [x] `props_supervisor_options.rs` → `props/supervisor_options.rs`
- [x] `spawn_error.rs` → `spawn/spawn_error.rs`
- [x] `name_registry.rs` → `spawn/name_registry.rs`
- [x] `name_registry_error.rs` → `spawn/name_registry_error.rs`
- [x] `system.rs` → `system/root.rs`（`system.rs` では `mod root;` 宣言し `pub use root::*;` で再エクスポート）
- [x] `system_state.rs` → `system/system_state.rs`
- [x] `dispatcher.rs` → `system/dispatcher.rs`
- [x] `event_stream*.rs` → `eventstream/`
- [x] `lifecycle_event.rs`, `lifecycle_stage.rs` → `lifecycle/`
- [x] `deadletter*.rs` → `deadletter/`
- [x] `log_event.rs`, `log_level.rs`, `logger_*` → `logging/`
- [x] `actor_future*.rs` → `futures/`
- [x] `actor_error*.rs`, `send_error.rs` → `error/`
- [x] 関連するテスト・ベンチファイルも移動（`tests/`、`benches/`）
- [x] `./scripts/ci-check.sh all`

### 3. 内部実装の可視性調整とモジュール構造の修正
- [x] ルートファイルを Module Wiring 規約に準拠（`mod` 宣言 + `pub use` 再エクスポート）
- [x] すべてのインポートパスを新しい階層パスへ更新（actor-core内部およびactor-std、examples、tests含む）
- [x] module-wiring-lint の全66エラーを修正
- [x] `./scripts/ci-check.sh all`

### 4. `lib.rs` の更新
- [x] 新しいモジュール宣言 (`pub mod actor_prim;` など) を追加
- [x] 旧来のフラットな再エクスポートを削除し、新しい階層パスのみを公開
- [x] **`prelude` から必要な再エクスポートを提供せずに**、内部からの FQCN 利用へ統一(`prelude` は完全にエンドユーザ用です)
- [x] `./scripts/ci-check.sh all`

## フェーズ2: ビルド・テスト
- [x] `cargo fmt`
- [x] `./scripts/ci-check.sh all`（全テスト、全リント、全チェックが通過）

## フェーズ3: ドキュメント整備
- [ ] `docs/guides/module_wiring.md` に新構造の説明を追記（オプション）
- [x] 新しいパッケージ構造に沿った API ドキュメントコメントを各ルートファイルに追加
- [ ] 利用者向け `prelude` のリストを更新（別タスクで対応）
- [ ] 旧→新インポートパス対応表を作成（マイグレーションガイド）（オプション）

## フェーズ4: OpenSpec 検証
- [x] `openspec validate refactor-actor-core-package-structure --strict`
- [x] すべてのチェックが通過し、実装完了
