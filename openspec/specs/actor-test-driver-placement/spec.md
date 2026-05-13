# actor-test-driver-placement Specification

## Purpose
TBD - created by archiving change step03-move-test-tick-driver-to-adaptor-std. Update Purpose after archive.
## Requirements
### Requirement: std 依存のテストドライバおよびテストヘルパは actor-adaptor-std 側に配置されなければならない

std 依存の公開テストドライバおよび公開テストヘルパは actor-adaptor-std 側に配置されなければならない (MUST)。

`fraktor-actor-*` workspace において、`std::thread` / `std::time::Instant` / tokio 等の std 環境固有機能に依存する
公開テスト向けの TickDriver 実装および actor system test helper は、no_std クレートである
`fraktor-actor-core-kernel-rs` ではなく `fraktor-actor-adaptor-std-rs` 側に配置されなければならない(MUST)。

`fraktor-actor-core-kernel-rs` 側は以下のみを提供する(MUST):

- no_std で動作する抽象（`TickDriver` trait、`TickFeed`、`SchedulerTickExecutor` 等）
- `#[cfg(test)]` 配下の inline unit tests に必要な crate-private fixture

actor-core-kernel の crate-private fixture は inline unit tests 専用であり、公開 API、re-export、cross-crate test
helper として露出してはならない(MUST NOT)。この例外は actor-core-kernel の dev-cycle 制約を避けるためのもので、
actor-adaptor-std が公開 test helper を所有する方針を弱めない。

`fraktor-actor-adaptor-std-rs` 側は以下を提供する(MUST):

- std 環境固有の TickDriver 実装（`StdTickDriver`、`TokioTickDriver`、`TestTickDriver` など）
- std 環境を前提とした test helper（`create_noop_actor_system` / `create_noop_actor_system_with<F>`）

actor-adaptor-std の test helper は actor-core-kernel の private construction seam に依存してはならず(MUST NOT)、
`ActorSystem::create_with_noop_guardian` 経由で bootstrapped system を作らなければならない(MUST)。

#### Scenario: TestTickDriver の公開定義は actor-adaptor-std 側にのみ存在する

- **WHEN** workspace の `modules/actor-*/src/**/*.rs` で `pub struct TestTickDriver` の定義を検査する
- **THEN** `modules/actor-adaptor-std/src/tick_driver/test_tick_driver.rs` にのみ存在する
- **AND** actor-core-kernel 側に公開可視性の `TestTickDriver` 定義は存在しない
- **AND** actor-core-kernel は `TestTickDriver` を公開 re-export しない

#### Scenario: std 依存の actor system test helper の公開 API は actor-adaptor-std 側にのみ存在する

- **WHEN** actor-core-kernel の `impl ActorSystem` を検査する
- **THEN** `pub fn new_empty` / `pub fn new_empty_with` / `pub fn new_noop` / `pub fn new_noop_with` は存在しない
- **AND** std 依存 test helper は `fraktor_actor_adaptor_std_rs::system::create_noop_actor_system` /
  `create_noop_actor_system_with<F>` として提供される

#### Scenario: std test helper は actor-core construction bypass を使わない

- **WHEN** `modules/actor-adaptor-std/src/system` の helper 実装を検査する
- **THEN** helper は `TestTickDriver` と std mailbox clock を設定する
- **AND** `ActorSystem::create_with_noop_guardian` を呼ぶ
- **AND** `ActorSystem::from_state`、`ActorSystem::create_started_from_config`、`SystemStateShared::new(SystemState::new())`
  を呼ばない

#### Scenario: downstream crate は actor-adaptor-std の new noop helper を使う

- **WHEN** `fraktor-actor-*` workspace 内の downstream crate tests が test actor system を必要とする
- **THEN** `fraktor_actor_adaptor_std_rs::system::create_noop_actor_system` または
  `create_noop_actor_system_with` を import する
- **AND** `new_empty_actor_system` を import しない
- **AND** actor-core-kernel の internal constructor を呼ばない

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
