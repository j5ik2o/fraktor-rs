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

- `actor-core/Cargo.toml` の `[dev-dependencies]` に `fraktor-actor-adaptor-std-rs = { ..., features = ["test-support"] }` が記述され、actor-core の `#[cfg(test)]` インラインテストおよび `tests/*.rs` 統合テストから adaptor-std の TestTickDriver / new_empty* を利用できるようになること（Cargo の dev-cycle は prod 循環ではないため許容される）
- step04 以降で新設される専用テストヘルパ crate（`fraktor-actor-test-rs` 等）が std 非依存のテストヘルパを提供すること（本 requirement は std 依存部分の配置のみを対象とする）

#### Scenario: TestTickDriver は actor-adaptor-std 側にのみ定義される

- **WHEN** workspace の `modules/actor-*/src/**/*.rs` で `pub struct TestTickDriver` の定義を検査する
- **THEN** `modules/actor-adaptor-std/src/std/tick_driver/test_tick_driver.rs` にのみ存在する
- **AND** `modules/actor-core/src/` 配下には `TestTickDriver` の構造体定義、`mod test_tick_driver;` 宣言、および `pub use test_tick_driver::TestTickDriver;` 再エクスポートが存在しない
- **AND** `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver.rs` は `TestTickDriver` を再エクスポートしない

#### Scenario: std 依存のテストコンストラクタは actor-core の ActorSystem メソッドから外される

- **WHEN** `modules/actor-core/src/core/kernel/system/base.rs` の `impl ActorSystem` ブロックを検査する
- **THEN** `pub fn new_empty(...)` および `pub fn new_empty_with<F>(...)` メソッドは存在しない
- **AND** `#[cfg(any(test, feature = "test-support"))]` ゲート内で `TestTickDriver::default()` を参照する公開メソッドは存在しない
- **AND** 同等の機能は `fraktor_actor_adaptor_std_rs::std::system::new_empty_actor_system` / `new_empty_actor_system_with<F>` 自由関数として提供される

#### Scenario: actor-core は std 依存テストヘルパ利用時に actor-adaptor-std を dev-dependency 経由で参照する

- **WHEN** `modules/actor-core/Cargo.toml` の `[dev-dependencies]` セクションを検査する
- **THEN** `fraktor-actor-adaptor-std-rs` エントリが存在し `features = ["test-support"]` が有効化されている
- **AND** `[dependencies]` セクションには `fraktor-actor-adaptor-std-rs` が含まれない（prod 依存としての循環は禁止、dev 依存としての循環は Cargo が許容する）

#### Scenario: 下流クレートは actor-adaptor-std 経由で TestTickDriver を利用する

- **WHEN** `fraktor-actor-*-rs` workspace 内の任意の crate のテストコード（`tests/*.rs` および `src/**/*tests.rs`）が `TestTickDriver` を利用する
- **THEN** `use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;` 形式で import する
- **AND** `use fraktor_actor_core_rs::...::TestTickDriver;` 形式の import は存在しない
