# プロジェクト構造
> 最終更新: 2025-11-17

## 組織方針
- ワークスペースは `modules/utils`（`fraktor-utils-rs`）、`modules/actor`（`fraktor-actor-rs`）、`modules/remote`（`fraktor-remote-rs`）の 3 クレートで構成され、各クレートが `core`（default `#![no_std]`）と `std` モジュールを持つ 2018 モジュール構成です。依存方向は utils/core → actor/core → actor/std → remote/core → remote/std の一方通行に固定します。
- 各モジュールは 2018 エディションのファイルツリー（`foo.rs` + `foo/` ディレクトリ）で構成し、`mod.rs` を使用しません。
- `type-per-file-lint` により 1 ファイル 1 構造体/trait を原則とし、テストは `hoge/tests.rs` へ分離します。
- 公開 API に限り `prelude` を許容し、内部は FQCN (`crate::...`) で明示的に依存をたどります。
- std 向けの追加コードは `modules/actor/src/std/*` へ閉じ込め、`cfg-std-forbid-lint` により core 内の `#[cfg(feature = "std")]` 分岐を禁止します。

## ディレクトリパターン
### ランタイムクレート階層
**Location**: `modules/utils/src/{core,std}`, `modules/actor/src/{core,std}`, `modules/remote/src/{core,std}`
**Purpose**: `fraktor-utils-rs::core` が RuntimeToolbox/Atomic/Timer を提供し、`fraktor-actor-rs::core` が ActorSystem/Mailbox/Remoting の基盤を no_std で構築、`std` モジュールが Tokio 実行器・ログ・Dispatcher を後掛けします。`fraktor-remote-rs::core` は Remoting 拡張・Endpoint/Watcher/Transport 抽象を actor/core の上に載せ、`std` 側が Tokio TCP などホスト固有のトランスポートを束ねる直列構造です。
**Example**: `modules/actor/src/core/messaging/*.rs` がコアメッセージング、`modules/actor/src/std/messaging/*.rs` がホスト固有の同名モジュールを実装。`modules/remote/src/std/transport/tokio_tcp/*.rs` が std 向け TCP トランスポート実装。

### ドメインモジュール
**Location**: `modules/actor/src/core/<domain>/`
**Purpose**: ActorCell, Mailbox, Supervision, Typed API などドメイン単位でサブディレクトリを持ち、`actor_context.rs` + `actor_context/` のように entry ファイルと詳細ファイルを分離。
**Example**: `modules/actor/src/core/actor_prim/actor/tests.rs` にドメイン専用テストを配置。

### std 向けバインディング
**Location**: `modules/actor/src/std/*`
**Purpose**: Tokio Executor / EventStream adapter / ActorSystem wrapper / TickDriver bootstraper を std 専用モジュールに閉じ込める。`std/system/actor_system_builder.rs` が builder パターンを提供し、`std/scheduler/tick.rs` が TickDriverConfig ヘルパを持ちます。
**Example**: `modules/actor/src/std/system/base.rs` が Core ActorSystem を包む `ActorSystem` 型を提供。

### リモートアドレッシング & Authority
**Location**: `modules/actor/src/core/actor_prim/actor_path/*`, `modules/actor/src/core/system/remote_authority.rs`
**Purpose**: `parts.rs`（`ActorPathParts`・`GuardianKind`）、`formatter.rs`、`path.rs` を分けて canonical URI 生成を単一責務化し、`RemoteAuthorityManagerGeneric` が remoting の状態管理（Unresolved/Connected/Quarantine）と deferred キューの排出を担います。
**Example**: `actor_prim/actor_selection/tests.rs` が guardian を越えない相対解決シナリオを網羅し、`system/remote_authority/tests.rs` が quarantine/手動解除/InvalidAssociation を `tests.rs` に閉じ込めています。

### スケジューラ & Tick Driver
**Location**: `modules/actor/src/core/scheduler/tick_driver/*`, `modules/actor/src/std/scheduler/tick.rs`, `docs/guides/tick-driver-quickstart.md`
**Purpose**: TickDriver 抽象・Bootstrap・SchedulerTickExecutor をコア側で定義し、Tokio/embedded/manual driver を同じ API で選択。ガイドは Quickstart/embedded/manual を 1 か所にまとめ、コード追加時に表と仕様を同期する。
**Example**: `tick_driver_matrix.rs` がドライバ一覧を管理し、`docs/guides/tick-driver-quickstart.md` が `StdTickDriverConfig::tokio_quickstart*` サンプルを提供。

### ドキュメント & ガイド
**Location**: `docs/guides`
**Purpose**: ActorSystem 運用や DeathWatch/TickDriver 移行など運用パターンを文章化し、spec ではなく作業ガイドとして参照。
**Example**: `docs/guides/tick-driver-quickstart.md` が TickDriver のシナリオ別導入手順を管理、`docs/guides/actor-system.md` が no_std / std 共通の初期化を示す。

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
- **クレート名**: 既存は `fraktor-utils-rs`, `fraktor-actor-rs`, `fraktor-remote-rs`, `fraktor-rs`。新規クレートも `fraktor-<domain>-rs` を踏襲し、Cargo features は `kebab-case`（例: `alloc-metrics`, `tokio-executor`）。
- **ドキュメント言語**: rustdoc は英語、それ以外のコメント・Markdown は日本語。
- **ActorPath 初期値**: `ActorPath::root()` は system 名に `cellactor` を用い、guardian は `GuardianKind::User/System` から自動付与するため、手動で `/cellactor` を記述しないこと。
- **Authority 表記**: リモート authority は `host:port` 文字列で `RemoteAuthorityManager` のキーにし、`PathAuthority` 経由で host/port を保持する。命名は小文字 + `-` を基本とし、実ホスト名を抽象化します。

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
- `fraktor-actor-rs::core`/`fraktor-utils-rs::core` は `#![no_std]` でビルドし、`cfg-std-forbid` lint で `#[cfg(feature = "std")]` を禁止。std 依存機能は `pub mod std` 以下（feature 有効時のみコンパイル）へ隔離します。
- Lifecycle 系統は system mailbox に `SystemMessage` を投げ入れ、ユーザメッセージより優先して処理することで determinism を担保。
- DeathWatch/監督/ログ/DeadLetter は EventStream を介して疎結合化し、観測面の利用者が自由に購読可能。
- テストは `hoge/tests.rs`（単体）と `modules/actor/tests/*.rs`（統合）に分け、`tests-location-lint` で逸脱を検出します。
- 新規 capability は OpenSpec (requirements → design → tasks) を通して合意し、ステアリングはパターン変化が生じたときのみ更新します。
- Remoting への追記は `system::remote_authority` 経由で一元化し、ActorPath 側の guardian/authority パターン（`pekko` / `pekko.tcp` スキーム）との乖離が出ないように spec（`pekko-compatible-actor-path`）で検証してから着手します。
- TickDriver 関連コードは `core/scheduler/tick_driver/*`（抽象）と `std/scheduler/tick.rs`（Tokio), docs/guides/tick-driver-quickstart.md（ドキュメント）を同時に更新し、`modules/actor/tests/system_*` でカバレッジを追加します。

---
_構造パターンを記録し、新しいファイルはここに記載したルールへ従う限り自由に追加できます。_
