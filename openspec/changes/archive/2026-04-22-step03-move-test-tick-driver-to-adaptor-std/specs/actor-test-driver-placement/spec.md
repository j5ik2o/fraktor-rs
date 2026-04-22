## ADDED Requirements

### Requirement: std 依存のテストドライバおよびテストヘルパは actor-adaptor-std 側に配置されなければならない

`fraktor-actor-*` workspace において、`std::thread` / `std::time::Instant` / tokio 等の std 環境固有機能に依存するテスト向けの TickDriver 実装およびテストユーティリティは、no_std クレートである `fraktor-actor-core-rs` ではなく `fraktor-actor-adaptor-std-rs` 側に配置されなければならない（MUST）。

`fraktor-actor-core-rs` 側は以下のみを提供する（MUST）:

- no_std で動作する抽象（`TickDriver` trait、`TickFeed`、`SchedulerTickExecutor`、`TickDriverBootstrap` 等）
- `#[cfg(test)]` 配下の inline ユニットテスト（actor-core 自身のテストとして、外部には公開しない）

`fraktor-actor-adaptor-std-rs` 側は以下を提供する（MUST）:

- std 環境固有の TickDriver 実装（`StdTickDriver`、`TokioTickDriver`、`TestTickDriver` など）
- std 環境を前提としたテスト専用のコンストラクタ（`new_empty_actor_system` / `new_empty_actor_system_with<F>` など、`TestTickDriver` を内包するもの）

将来同種のケース（std 依存のテストヘルパが no_std クレートに同居している状態）が検出された場合は、本 requirement に従って adaptor 層へ移設する。

本 requirement は以下を許容する（例外）:

- `actor-core/Cargo.toml` の `[dev-dependencies]` に `fraktor-actor-adaptor-std-rs = { ..., features = ["test-support"] }` が記述され、actor-core の `tests/*.rs` 統合テストから adaptor-std の TestTickDriver / new_empty* を利用できるようになること（Cargo の dev-cycle は prod 循環ではないため許容される）
- step04 以降で新設される専用テストヘルパ crate（`fraktor-actor-test-rs` 等）が std 非依存のテストヘルパを提供すること（本 requirement は std 依存部分の配置のみを対象とする）
- **dev-cycle workaround として、actor-core 内部にのみ可視な `pub(crate)` 限定の `TestTickDriver` / `new_empty*` 重複実装を残すこと**。Cargo の dev-cycle 制約により、actor-core の inline test (`src/**/tests.rs`) からは adaptor-std::TestTickDriver を参照すると同一クレート (`fraktor_actor_core_rs`) が二バージョンとして compiler に見え型不一致になる（Rust/Cargo の根本仕様、回避不能）。この内部版は **公開 API には現れず** (`pub(crate)` 限定、test-support feature 非経由)、`#[cfg(test)]` ゲートで通常ビルドからも除外される。最終撤去は inline test を統合テストに移行する後続 change で行う

#### Scenario: TestTickDriver の公開定義は actor-adaptor-std 側にのみ存在する

- **WHEN** workspace の `modules/actor-*/src/**/*.rs` で `pub struct TestTickDriver` の定義を検査する
- **THEN** `modules/actor-adaptor-std/src/std/tick_driver/test_tick_driver.rs` にのみ存在する
- **AND** `modules/actor-core/src/` 配下に `pub struct TestTickDriver`（公開可視性）の定義は存在しない
- **AND** `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver.rs` は `TestTickDriver` を再エクスポートしない（公開も pub(crate) 含めて）

**dev-cycle workaround 例外**: `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tests/test_tick_driver.rs` には `pub(crate) struct TestTickDriver` が存在するが、これは `tick_driver/tests.rs` の `#![cfg(test)]` (file-level inner attribute) でゲートされた test-only モジュールに置かれており、通常ビルドにも公開 API にも現れない。本 requirement の許容例外として明示的にスコープ外とする。

#### Scenario: std 依存のテストコンストラクタの公開 API は actor-core から外される

- **WHEN** `modules/actor-core/src/core/kernel/system/base.rs` の `impl ActorSystem` ブロックを検査する
- **THEN** `pub fn new_empty(...)` および `pub fn new_empty_with<F>(...)` の公開メソッドは存在しない
- **AND** `#[cfg(any(test, feature = "test-support"))]` ゲート（公開 feature 経由）内で `TestTickDriver::default()` を参照する公開メソッドは存在しない
- **AND** 同等の公開機能は `fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system` / `new_empty_actor_system_with<F>` 自由関数として提供される

**dev-cycle workaround 例外**: `modules/actor-core/src/core/kernel/system/base/tests.rs` 内の `impl ActorSystem` ブロックに `pub(crate) fn new_empty()` / `pub(crate) fn new_empty_with<F>()` が存在するが、これは test-only ファイル (`tests.rs` で `#![cfg(test)]` 配下) に置かれた crate 内部限定 API であり、公開 API にも通常ビルドにも現れない。`TypedActorSystem<M>` についても同様に `modules/actor-core/src/core/typed/system/tests.rs` 内に `pub(crate) fn new_empty()` が存在する。本 requirement の許容例外として明示的にスコープ外とする。

#### Scenario: actor-core は std 依存テストヘルパ利用時に actor-adaptor-std を dev-dependency 経由で参照する

- **WHEN** `modules/actor-core/Cargo.toml` の `[dev-dependencies]` セクションを検査する
- **THEN** `fraktor-actor-adaptor-std-rs` エントリが存在し `features = ["test-support"]` が有効化されている
- **AND** `[dependencies]` セクションには `fraktor-actor-adaptor-std-rs` が含まれない（prod 依存としての循環は禁止、dev 依存としての循環は Cargo が許容する）

#### Scenario: 下流クレートおよび actor-core integration test は actor-adaptor-std 経由で TestTickDriver を利用する

- **WHEN** `fraktor-actor-*-rs` workspace 内の crate のテストコードが `TestTickDriver` を利用する。スコープは以下:
  - 下流 crate (`cluster-*`、`stream-*`、`persistence-*`、`actor-adaptor-std` 自身) の `tests/*.rs` および `src/**/*tests.rs`
  - `actor-core` の **integration test** (`modules/actor-core/tests/*.rs`)
- **THEN** `use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;` 形式で import する
- **AND** `use fraktor_actor_core_rs::...::TestTickDriver;` 形式の import は存在しない

**dev-cycle workaround 例外**: `actor-core` の inline test (`modules/actor-core/src/**/tests.rs`) は同一クレート二バージョン問題を避けるため `crate::core::kernel::actor::scheduler::tick_driver::tests::TestTickDriver` (`pub(crate)` 内部版) を利用する。これは本 Scenario のスコープ外。
