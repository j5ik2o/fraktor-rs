## MODIFIED Requirements

### Requirement: actor-* の Cargo.toml は primitive lock crate を non-optional な直接依存として宣言してはならない

`actor-*` クレートの `Cargo.toml` は、`critical-section`、`spin`、`parking_lot` などの primitive lock crate を `[dependencies]` に直接依存として宣言してはならない（MUST NOT）。これらの crate への依存は、`utils-core` を通した推移的依存として表現されなければならない（MUST）。

ただし以下は例外として許可する:

- `portable-atomic` のような low-level utility crate が引き込む推移的依存
- 各クレートの `[dev-dependencies]` に test/bench 用の impl provider 取得目的で記述される `critical-section = { features = ["std"] }` 等のエントリ（production 依存ではないため `[dependencies]` 直接宣言禁止には該当しない）
- `showcases/std` のような実行バイナリ crate の `[dependencies]` における impl provider 取得目的の記述（バイナリ側が impl 選択責任を持つ標準作法に基づく）

impl provider 取得は各バイナリが `[dev-dependencies]` または `[dependencies]` で直接宣言する形に統一する。

#### Scenario: actor-core の Cargo.toml は critical-section を `[dependencies]` 直接依存として持たない

- **WHEN** `modules/actor-core/Cargo.toml` の `[dependencies]` セクションで `critical-section` エントリを検査する
- **THEN** `critical-section` エントリは存在しない
- **AND** `critical-section` への依存は `portable-atomic = { features = ["critical-section"] }` のような推移的経路でのみ表現される
- **AND** `[dev-dependencies]` には `critical-section = { workspace = true, features = ["std"] }` が impl provider 取得目的で記述されてよい（actor-core 自身の `cargo test` で必要）

#### Scenario: actor-* の他クレートも同じ規約に従う

- **WHEN** `fraktor-actor-adaptor-std-rs`、`fraktor-cluster-*-rs`、`fraktor-remote-*-rs`、`fraktor-stream-*-rs`、`fraktor-persistence-*-rs` の `Cargo.toml` を読む
- **THEN** いずれも `critical-section`、`spin`、`parking_lot` を `[dependencies]` 直接宣言として持たない
- **AND** これらのクレートが同期プリミティブを必要とする場合は `fraktor-utils-core-rs` 経由で取得する
- **AND** test/bench で `critical-section` の `std` impl が必要な場合は `[dev-dependencies]` に `critical-section = { workspace = true, features = ["std"] }` を直接記述する

#### Scenario: 各バイナリは impl provider を直接宣言する

- **WHEN** `actor-*` 配下のテスト（`[[test]]`）、bench、または `showcases/std` 等の実行バイナリ crate が `critical-section` の impl を必要とする
- **THEN** 当該 crate は `[dev-dependencies]` または `[dependencies]` に `critical-section = { workspace = true, features = ["std"] }` を直接記述する
- **AND** `actor-*` の library crate の feature flag（例: `test-support`）を経由した自動配給には依存しない
