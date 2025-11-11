# プロジェクト構造
> 最終更新: 2025-11-08

## 組織方針
- ワークスペースは `modules/` 以下に no_std コア (`utils-core`, `actor-core`) と std 補助 (`utils-std`, `actor-std`) を縦方向に積み上げ、依存方向を一方向（utils → actor → アプリケーション）へ固定します。
- 各モジュールは 2018 エディションのファイルツリー（`foo.rs` + `foo/` ディレクトリ）で構成し、`mod.rs` を使用しません。
- `type-per-file-lint` により 1 ファイル 1 構造体/trait を原則とし、テストは `hoge/tests.rs` へ分離します。
- 公開 API に限り `prelude` を許容し、内部は FQCN (`crate::...`) で明示的に依存をたどります。

## ディレクトリパターン
### ランタイムクレート階層
**Location**: `modules/utils-core`, `modules/actor-core`, `modules/actor-std`, `modules/utils-std`
**Purpose**: no_std の同期/所有権プリミティブ → no_std ActorSystem → std 連携（Tokio, ホストログ）→ std 補助という依存鎖を形成。
**Example**: `modules/actor-core/src/messaging/*.rs` がコアメッセージング、`modules/actor-std/src/messaging/*.rs` がホスト固有の同名モジュールを実装。

### ドメインモジュール
**Location**: `modules/actor-core/src/<domain>/`
**Purpose**: ActorCell, Mailbox, Supervision, Typed API などドメイン単位でサブディレクトリを持ち、`actor_context.rs` + `actor_context/` のように entry ファイルと詳細ファイルを分離。
**Example**: `modules/actor-core/src/actor_prim/actor/tests.rs` にドメイン専用テストを配置。

### ドキュメント & ガイド
**Location**: `docs/guides`
**Purpose**: ActorSystem 運用や DeathWatch 移行など運用パターンを文章化し、spec ではなく作業ガイドとして参照。
**Example**: `docs/guides/actor-system.md` が no_std / std 共通の初期化と観測手順を示す。

### Lint パッケージ
**Location**: `lints/<lint-name>`
**Purpose**: Dylint ベースで構造ルール（`mod-file`, `module-wiring`, `tests-location`, `cfg-std-forbid` など）をコンパイル時に適用。
**Example**: `lints/module-wiring-lint` が FQCN import と末端モジュールのみ再エクスポートする方針を強制。

### CI / スクリプト
**Location**: `scripts/*.sh`
**Purpose**: `ci-check.sh` を中心に lint / dylint / clippy / no_std / std / doc / embedded を一括実行し、再現性のある検証手順を共有。
**Example**: `scripts/ci-check.sh embedded` が `embassy` と `thumb` ターゲットをまとめてビルド。

## 命名規則
- **ファイル**: `snake_case.rs`。モジュール本体は `foo.rs`、実装詳細は `foo/bar.rs`、テストは `foo/tests.rs`。
- **ディレクトリ**: `snake_case/`。`foo.rs` に対応する `foo/` を置き、サブモジュールを格納。
- **型 / トレイト**: `PascalCase`。trait 名は `*Ext` や `*Service` 等の役割語尾を避け、ドメイン名を直截に記述。
- **モジュール境界**: 1 ファイル 1 型（構造体または trait）を基本とし、補助型は `tests.rs` かサブモジュールへ退避。
- **クレート名**: `fraktor-<domain>-rs`。Cargo features は `kebab-case`（例: `alloc-metrics`, `tokio-executor`）。
- **ドキュメント言語**: rustdoc は英語、それ以外のコメント・Markdown は日本語。

## import 組織
```rust
use crate::actor_prim::actor_ref::ActorRef;
use crate::system::system_message::SystemMessage; // FQCN で辿り、末端モジュールのみ再エクスポート

// Prelude はユーザ公開 API のみ
pub mod prelude {
    pub use crate::messaging::message::AnyMessage;
}
```
**パスエイリアス**:
- なし。Rust 2018 の `crate::` / `super::` / `self::` を必須とし、`module-wiring-lint` が暗黙エイリアスを禁止。

## コード組織の原則
- `actor-core`/`utils-core` は `#![no_std]` でビルドし、`cfg-std-forbid` lint で `#[cfg(feature = "std")]` を排除。std 依存機能は対応する `*-std` クレートで定義します。
- Lifecycle 系統は system mailbox に `SystemMessage` を投げ入れ、ユーザメッセージより優先して処理することで determinism を担保。
- DeathWatch/監督/ログ/DeadLetter は EventStream を介して疎結合化し、観測面の利用者が自由に購読可能。
- テストは `hoge/tests.rs`（単体）と `crate/tests/*.rs`（統合）に分け、`tests-location-lint` で逸脱を検出します。
- 新規 capability は OpenSpec (requirements → design → tasks) を通して合意し、ステアリングはパターン変化が生じたときのみ更新します。

---
_構造パターンを記録し、新しいファイルはここに記載したルールへ従う限り自由に追加できます。_
