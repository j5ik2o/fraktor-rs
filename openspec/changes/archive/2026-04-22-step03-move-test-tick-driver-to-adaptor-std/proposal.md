## Why

`actor-core` の `test-support` feature が抱える「責務 B（ダウンストリーム統合テスト用 API 公開）」のうち、std 環境依存の中核コンポーネント群は:

- `TestTickDriver`（`std::thread` + `thread::sleep` で tick を駆動するテストドライバ）
- `ActorSystem::new_empty` / `new_empty_with`（内部で `TestTickDriver::default()` を参照する std 依存コンストラクタ）

`actor-core` は no_std クレートであるため、これらの std 依存ヘルパを本体で提供していること自体が責務の漏洩である。`actor-adaptor-std` は既に std 環境専用のアダプタクレートとして存在し、`tokio-executor` feature / `test-support` feature を持ち、std 向けの周辺機能（`StdTickDriver`、`TokioTickDriver` 等）を集約する場所として位置づけられている。std 依存のテストヘルパはこちらに引っ越すのが構造的に自然。

なお `TestTickDriver` と `new_empty*` は `base.rs:86` で `TestTickDriver::default()` を直接参照しており **分離不能** なため、本 change で同時に移設する（design Decision 2 で詳述）。

本 change は `test-support` feature を最終的に退役する長期計画（Strategy B）の第 3 ステップ。責務 A（`critical-section/std` impl provider）は既に退役済み（PR #1607/#1608）、責務 B-1（std 依存ヘルパ: `TestTickDriver` + `new_empty*`）として本 change で移設を行う。残る責務 B（mock / probe / その他ヘルパ）は step04 で対応する。

## What Changes

- `modules/actor-core/src/` 配下で `TestTickDriver` 関連の実装（構造体定義、テストユーティリティ、`#[cfg(any(test, feature = "test-support"))]` ゲート内の公開シンボル）を抽出
- `modules/actor-adaptor-std/src/std/tick_driver/` 配下（既存 `StdTickDriver` / `TokioTickDriver` と同階層、design で最終決定）に移設
- `ActorSystem::new_empty` / `new_empty_with` も **同時に** `actor-adaptor-std` 側へ移設する（これらは `TestTickDriver::default()` を内部で参照しており、`TestTickDriver` 移動と構造的に分離不能）。自由関数 `new_empty_actor_system` / `new_empty_actor_system_with<F>` として提供
- `actor-core` 側では no_std セーフな `TickDriver` trait と `TickDriverBootstrap` など抽象インフラのみを残す
- ダウンストリーム（`showcases/std`、`actor-core` 自身の `[[test]]`、他 crate の test、`actor-core` 自身の `src/**/tests.rs` インラインテスト）の import path を更新
- `actor-core/test-support` feature からは `TestTickDriver` と `new_empty*` 関連が消えるため、残責務 B の範囲が縮小（残りは step04 で対応）
- `actor-core` の `[dev-dependencies]` に `fraktor-actor-adaptor-std-rs = { workspace = true, features = ["test-support"] }` を追加（Cargo は dev-cycle を許容するため、`actor-core` のテストから `actor-adaptor-std` 経由で `TestTickDriver` を利用可能にする）
- **BREAKING（workspace-internal）**:
  - `fraktor_actor_core_rs::...::TestTickDriver` → `fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver`
  - `ActorSystem::new_empty()` / `ActorSystem::new_empty_with(...)` → `fraktor_actor_adaptor_std_rs::...::new_empty_actor_system()` / `new_empty_actor_system_with(...)`

**Non-Goals**:
- `new_empty*` 以外の test-support 公開 API（`MockActorRef`、`TestProbe` 等の mock / fixture、responsibility B-2 の残り）の移設は step04 で行う
- `test-support` feature 自体の削除は step06 で行う
- `actor-adaptor-std` の既存 `test-support` feature 設計見直し（現状の構造を尊重）

## Capabilities

### New Capabilities
- `actor-test-driver-placement`: 「std 依存のテストドライバおよび std 依存のテストヘルパは `fraktor-actor-core-rs`（no_std クレート）ではなく `fraktor-actor-adaptor-std-rs` 側に置く」原則を明文化する capability を新設（design / specs で詳細確定）

### Modified Capabilities
- なし

## Impact

- **Affected code**:
  - `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/test_tick_driver.rs`（削除）
  - `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver.rs`（module 宣言・`pub use` 削除）
  - `modules/actor-core/src/core/kernel/system/base.rs`（`new_empty` / `new_empty_with` 削除）
  - `modules/actor-adaptor-std/src/std/tick_driver/test_tick_driver.rs`（新規、TestTickDriver 実装）
  - `modules/actor-adaptor-std/src/std/tick_driver.rs`（新規モジュールの `pub use` 追加）
  - `modules/actor-adaptor-std/src/std/system/empty_system.rs`（新規、`new_empty_actor_system` / `new_empty_actor_system_with` 自由関数）または同等の配置（design で確定）
  - `modules/actor-adaptor-std/Cargo.toml`（変更なし想定。現状の `test-support = ["fraktor-actor-core-rs/test-support"]` を維持し、新規 `TestTickDriver` / `new_empty*` はソース側の `#[cfg(feature = "test-support")]` gate のみで切り替え。実装フェーズで確認）
  - `modules/actor-core/Cargo.toml`（`[dev-dependencies]` に `fraktor-actor-adaptor-std-rs` 追加）
  - `modules/actor-core/src/**/*tests.rs`（インラインテスト 20+ ファイルの import path 更新）
  - `modules/actor-core/tests/*.rs`（統合テスト 8 ファイルの import path 更新）
  - `modules/cluster-core/`、`modules/stream-core/`、`modules/stream-adaptor-std/`、`modules/persistence-core/` 等のテスト import path 更新
  - `showcases/std/` 配下の example の import path 更新
- **Affected APIs**:
  - `TestTickDriver` のクレートパス変更（workspace-internal breaking）
  - `ActorSystem::new_empty` / `new_empty_with` メソッド削除 → 自由関数化
- **Affected dependencies**:
  - `actor-adaptor-std` の `test-support` feature が `TestTickDriver` / `new_empty*` を公開
  - `actor-core` の `[dev-dependencies]` に `actor-adaptor-std` が追加される（Cargo dev-cycle として許容）
- **Release impact**: pre-release phase につき外部影響は軽微
