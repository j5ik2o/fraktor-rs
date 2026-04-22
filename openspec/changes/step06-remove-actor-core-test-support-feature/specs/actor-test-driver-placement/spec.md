## MODIFIED Requirements

### Requirement: actor-core では feature ゲート経由で内部 API の可視性を拡大してはならない

`fraktor-actor-core-rs` の本体ソース (`modules/actor-core/src/**/*.rs`) では、`#[cfg(any(test, feature = "test-support"))]` のような **`feature = "test-support"` を含む cfg ゲート** を使って `pub(crate)` 以下の内部 API を `pub` に格上げしてはならない（MUST NOT）。

加えて、本 capability では **`fraktor-actor-core-rs` クレート自身が `test-support` feature を提供してはならない** (MUST NOT)。`actor-core/Cargo.toml` の `[features]` セクションに `test-support` の定義が存在してはならず、関連する `[[test]] required-features = ["test-support"]` も含まれてはならない。同様に、ダウンストリームクレートの `Cargo.toml` で `fraktor-actor-core-rs = { ..., features = ["test-support"] }` のように **存在しない feature を要求してはならない** (MUST NOT)。

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
