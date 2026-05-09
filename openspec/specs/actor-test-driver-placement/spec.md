# actor-test-driver-placement Specification

## Purpose
TBD - created by archiving change step03-move-test-tick-driver-to-adaptor-std. Update Purpose after archive.
## Requirements
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
- step04 close により責務 B-2 残（actor-core 内部 caller のみのテストヘルパ）は新 crate 化せず、本 capability の新規 Requirement に従って `pub(crate)` 化する（step05 で実施済み）
- **dev-cycle workaround として、actor-core 内部にのみ可視な `pub(crate)` 限定の `TestTickDriver` / `new_empty*` 重複実装を残すこと**。Cargo の dev-cycle 制約により、actor-core の inline test (`src/**/tests.rs`) からは adaptor-std::TestTickDriver を参照すると同一クレート (`fraktor_actor_core_rs`) が二バージョンとして compiler に見え型不一致になる（Rust/Cargo の根本仕様、回避不能）。この内部版は **公開 API には現れず** (`pub(crate)` 限定、test-support feature 非経由)、`#[cfg(test)]` ゲートで通常ビルドからも除外される。最終撤去は inline test を統合テストに移行する後続 change で行う

#### Scenario: TestTickDriver の公開定義は actor-adaptor-std 側にのみ存在する

- **WHEN** workspace の `modules/actor-*/src/**/*.rs` で `pub struct TestTickDriver` の定義を検査する
- **THEN** `modules/actor-adaptor-std/src/tick_driver/test_tick_driver.rs` にのみ存在する
- **AND** `modules/actor-core/src/` 配下に `pub struct TestTickDriver`（公開可視性）の定義は存在しない
- **AND** `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver.rs` は `TestTickDriver` を再エクスポートしない（公開も pub(crate) 含めて）

**dev-cycle workaround 例外**: `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tests/test_tick_driver.rs` には `pub(crate) struct TestTickDriver` が存在するが、これは `tick_driver/tests.rs` の `#![cfg(test)]` (file-level inner attribute) でゲートされた test-only モジュールに置かれており、通常ビルドにも公開 API にも現れない。本 requirement の許容例外として明示的にスコープ外とする。

#### Scenario: std 依存のテストコンストラクタの公開 API は actor-core から外される

- **WHEN** `modules/actor-core/src/core/kernel/system/base.rs` の `impl ActorSystem` ブロックを検査する
- **THEN** `pub fn new_empty(...)` および `pub fn new_empty_with<F>(...)` の公開メソッドは存在しない
- **AND** `#[cfg(any(test, feature = "test-support"))]` ゲート（公開 feature 経由）内で `TestTickDriver::default()` を参照する公開メソッドは存在しない
- **AND** 同等の公開機能は `fraktor_actor_adaptor_std_rs::system::new_empty_actor_system` / `new_empty_actor_system_with<F>` 自由関数として提供される

**dev-cycle workaround 例外**: `modules/actor-core/src/core/kernel/system/base/tests.rs` 内の `impl ActorSystem` ブロックに `pub(crate) fn new_empty()` / `pub(crate) fn new_empty_with<F>()` が存在するが、これは test-only ファイル (`tests.rs` で `#![cfg(test)]` 配下) に置かれた crate 内部限定 API であり、公開 API にも通常ビルドにも現れない。`TypedActorSystem<M>` についても同様に `modules/actor-core/src/core/typed/system/tests.rs` 内に `pub(crate) fn new_empty()` が存在する。本 requirement の許容例外として明示的にスコープ外とする。

#### Scenario: actor-core は std 依存テストヘルパ利用時に actor-adaptor-std を dev-dependency 経由で参照する

- **WHEN** `modules/actor-core/Cargo.toml` の `[dev-dependencies]` セクションを検査する
- **THEN** `fraktor-actor-adaptor-std-rs` エントリが存在し `features = ["test-support"]` が有効化されている
- **AND** `[dependencies]` セクションには `fraktor-actor-adaptor-std-rs` が含まれない（prod 依存としての循環は禁止、dev 依存としての循環は Cargo が許容する）

#### Scenario: 下流クレートおよび actor-core integration test は actor-adaptor-std 経由で TestTickDriver を利用する

- **WHEN** `fraktor-actor-*-rs` workspace 内の crate のテストコードが `TestTickDriver` を利用する。スコープは以下:
  - 下流 crate (`cluster-*`、`stream-*`、`persistence-*`、`actor-adaptor-std` 自身) の `tests/*.rs` および `src/**/*tests.rs`
  - `actor-core` の **integration test** (`modules/actor-core/tests/*.rs`)
- **THEN** `use fraktor_actor_adaptor_std_rs::tick_driver::TestTickDriver;` 形式で import する
- **AND** `use fraktor_actor_core_rs::...::TestTickDriver;` 形式の import は存在しない

**dev-cycle workaround 例外**: `actor-core` の inline test (`modules/actor-core/src/**/tests.rs`) は同一クレート二バージョン問題を避けるため `crate::core::kernel::actor::scheduler::tick_driver::tests::TestTickDriver` (`pub(crate)` 内部版) を利用する。これは本 Scenario のスコープ外。

### Requirement: actor-core では feature ゲート経由で内部 API の可視性を拡大してはならない

`fraktor-actor-core-rs` の本体ソース (`modules/actor-core/src/**/*.rs`) では、`#[cfg(any(test, feature = "test-support"))]` のような **`feature = "test-support"` を含む cfg ゲート** を使って `pub(crate)` 以下の内部 API を `pub` に格上げしてはならない（MUST NOT）。

加えて、本 capability では **`fraktor-actor-core-rs` クレート自身が `test-support` feature を提供してはならない** (MUST NOT)。`actor-core/Cargo.toml` の `[features]` セクションに `test-support` の定義が存在してはならず、関連する `[[test]] required-features = ["test-support"]` も含まれてはならない。同様に、ダウンストリームクレートの `Cargo.toml` で `fraktor-actor-core-rs = { ..., features = ["test-support"] }` のように **存在しない feature を要求してはならない** (MUST NOT)。

更に、**下流 (`actor-*` 系の library crate) が `test-support` feature を定義する場合、当該 crate の `src/**/*.rs` 内に `#[cfg(feature = "test-support")]` または `#[cfg(all(test, feature = "test-support"))]` のような実用ゲートを少なくとも 1 件持たなければならない** (MUST)。空 feature (`test-support = []` で src 内利用なし) や forward 専用 feature (`test-support = ["other_crate/test-support"]` で src 内利用なし) を残してはならない (MUST NOT)。

許容される使い方は以下のみ:

- 純粋な `#[cfg(test)]`（`feature = "test-support"` を含まない）による test-only コード分離
- `<module>/tests.rs` (file-level `#![cfg(test)]`) 配下に test fixture や test 用の inherent method を置くこと
- `pub(crate)` のままの内部 API（feature ゲートを伴わない）

「ダウンストリームのテストから internal API を叩きたい」という需要が現れた場合は、以下のいずれかで対応する（feature ゲート経由の visibility 拡大は禁止）:

- 当該 API を正規 public API として設計し、`pub` で公開する（docs / 型シグネチャを整備）
- ダウンストリームのテストを public API 経由 (`ActorRef::tell` 等) に書き換える
- `actor-adaptor-std` 等の adaptor crate にファサード関数を追加し、ダウンストリームはそれを経由する

#### Scenario: actor-core src 配下に test-support ゲート経由の visibility 拡大が存在しない

- **WHEN** `Grep "feature = \"test-support\"" modules/actor-core/src/` を実行する
- **THEN** ヒット件数が 0 件である
- **AND** `#[cfg(any(test, feature = "test-support"))]` 形式の attribute を本体 (`src/**/*.rs`) に持つ箇所が存在しない

#### Scenario: 内部 API の dual-visibility パターンが残存しない

- **WHEN** `modules/actor-core/src/` 配下の任意のファイルを検査する
- **THEN** 同一シンボルに対して以下のような dual-cfg pattern が存在しない:
  ```rust
  #[cfg(any(test, feature = "test-support"))]
  pub fn foo(...) { ... }
  #[cfg(not(any(test, feature = "test-support")))]
  pub(crate) fn foo(...) { ... }
  ```
- **AND** 該当シンボルは単一の `pub(crate)` 定義に統一されている

#### Scenario: `pub(crate)` 内部 API が常に存在する (test/test-support 限定の存在切替を行わない)

- **WHEN** `pub(crate)` 可視性を持つ内部 API のシンボルを検査する
- **THEN** `#[cfg(any(test, feature = "test-support"))]` で「test/test-support ビルド時のみ存在する」という存在切替を行っていない
- **AND** production caller (intra-crate) があるシンボルは常に存在し、ゲートで隠さない

#### Scenario: actor-core/Cargo.toml に test-support feature が存在しない

- **WHEN** `modules/actor-core/Cargo.toml` を検査する
- **THEN** `[features]` セクションに `test-support` の定義が存在しない
- **AND** `[[test]]` セクション群に `required-features = ["test-support"]` を含む行が 0 件
- **AND** 任意の `Grep "test-support" modules/actor-core/Cargo.toml` のヒットは、actor-adaptor-std を dev-dep として有効化する `fraktor-actor-adaptor-std-rs = { ..., features = ["test-support"] }` のみに限られる（actor-adaptor-std 側の独立 feature への参照）

#### Scenario: ダウンストリーム crate が actor-core の存在しない test-support feature を要求しない

- **WHEN** workspace 内の任意の `Cargo.toml` (`modules/**/Cargo.toml`、`showcases/**/Cargo.toml`) を検査する
- **THEN** `fraktor-actor-core-rs = { ..., features = [..., "test-support", ...] }` の形で actor-core の `test-support` を要求している行が存在しない
- **AND** 同様に他 crate の `test-support` feature 定義 (`test-support = [...]`) において `"fraktor-actor-core-rs/test-support"` を forward する記述が存在しない

#### Scenario: 下流 crate の test-support feature は実用ゲートを持つ場合のみ存在してよい

- **WHEN** `actor-*` 系の library crate (`modules/<crate>/Cargo.toml`) の `[features]` セクションに `test-support = [...]` 定義が存在する
- **THEN** 当該 crate の `src/**/*.rs` に `#[cfg(feature = "test-support")]` または `#[cfg(all(test, feature = "test-support"))]` のような実用ゲートが **少なくとも 1 件** 存在する
- **AND** 「forward only」(`test-support = ["other_crate/test-support"]` で当該 crate の src には 1 件もゲートがない) 状態は許されない
- **AND** 「空定義」(`test-support = []` で当該 crate の src には 1 件もゲートがない) 状態も許されない
- **AND** workspace 内で本 Scenario を満たす crate は `actor-adaptor-std` (`tick_driver.rs`、`std.rs`、`circuit_breakers_registry_id.rs` 等) のみであり、cluster-core / cluster-adaptor-std / remote-adaptor-std / persistence-core / stream-core / stream-adaptor-std には `test-support` feature 定義が存在しない
