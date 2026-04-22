## MODIFIED Requirements

### Requirement: actor-* の Cargo.toml は primitive lock crate を non-optional な直接依存として宣言してはならない

`actor-*` クレートの `Cargo.toml` は、`critical-section`、`spin`、`parking_lot` などの primitive lock crate を `[dependencies]` に直接依存として宣言してはならない（MUST NOT）。これらの crate への依存は、`utils-core` を通した推移的依存として表現されなければならない（MUST）。

ただし以下は例外として許可する:

- `portable-atomic` のような low-level utility crate が引き込む推移的依存
- 各クレートの `[dev-dependencies]` に test/bench 用の impl provider 取得目的で記述される `critical-section = { features = ["std"] }` 等のエントリ（production 依存ではないため `[dependencies]` 直接宣言禁止には該当しない）
- `showcases/std` のような実行バイナリ crate の `[dependencies]` における impl provider 取得目的の記述（バイナリ側が impl 選択責任を持つ標準作法に基づく）

impl provider 取得は各バイナリが `[dev-dependencies]` または `[dependencies]` で直接宣言する形に統一する。

加えて、`actor-*` クレートが low-level utility crate（`portable-atomic` 等）を直接依存として宣言する際に **特定 feature を指定する場合**、その feature 指定は単独で正当化されなければならない（MUST）。具体的には、Cargo.toml コメントまたは `docs/plan/` 配下の評価レポート（例: `docs/plan/YYYY-MM-DD-<topic>-evaluation.md`）への参照によって、その feature を有効化する **対象ターゲット** または **ユースケース** が明示されなければならない。「歴史的な理由でついている」状態の feature 指定を残してはならない（MUST NOT）。

#### Scenario: actor-core の Cargo.toml は critical-section を `[dependencies]` 直接依存として持たない

- **WHEN** `modules/actor-core/Cargo.toml` の `[dependencies]` セクションで `critical-section` エントリを検査する
- **THEN** `critical-section` エントリは存在しない
- **AND** `critical-section` への依存は `portable-atomic = { features = ["critical-section"] }` のような推移的経路でのみ表現される
- **AND** `[dev-dependencies]` には `critical-section = { workspace = true, features = ["std"] }` が impl provider 取得目的で記述されてよい（actor-core 自身の `cargo test` で必要）

#### Scenario: actor-core の Cargo.toml は spin を `[dependencies]` 直接依存として持たない

- **WHEN** `modules/actor-core/Cargo.toml` の `[dependencies]` セクションで `spin` エントリを検査する
- **THEN** `spin` エントリは存在しない
- **AND** `spin` への依存は `fraktor-utils-core-rs` 経由の推移的経路でのみ表現される
- **AND** `actor-core` の production code 内の write-once + lock-free read 用途（旧 `spin::Once<T>` 利用箇所）は `fraktor-utils-core-rs` が提供する `SyncOnce<T>` 抽象を通して構築される

#### Scenario: actor-* の他クレートも同じ規約に従う

- **WHEN** `fraktor-actor-adaptor-std-rs`、`fraktor-cluster-*-rs`、`fraktor-remote-*-rs`、`fraktor-stream-*-rs`、`fraktor-persistence-*-rs` の `Cargo.toml` を読む
- **THEN** いずれも `critical-section`、`spin`、`parking_lot` を `[dependencies]` 直接宣言として持たない
- **AND** これらのクレートが同期プリミティブを必要とする場合は `fraktor-utils-core-rs` 経由で取得する
- **AND** test/bench で `critical-section` の `std` impl が必要な場合は `[dev-dependencies]` に `critical-section = { workspace = true, features = ["std"] }` を直接記述する

#### Scenario: 各バイナリは impl provider を直接宣言する

- **WHEN** `actor-*` 配下のテスト（`[[test]]`）、bench、または `showcases/std` 等の実行バイナリ crate が `critical-section` の impl を必要とする
- **THEN** 当該 crate は `[dev-dependencies]` または `[dependencies]` に `critical-section = { workspace = true, features = ["std"] }` を直接記述する
- **AND** `actor-*` の library crate の feature flag（例: `test-support`）を経由した自動配給には依存しない

#### Scenario: low-level utility crate の feature 指定は対象ターゲット / ユースケースが明示されている

- **WHEN** `actor-*` クレートの `Cargo.toml` に `portable-atomic = { workspace = true, ..., features = [...] }` のような low-level utility crate の feature 指定が存在する
- **THEN** その feature 指定の正当化が以下のいずれかで残されている:
  - Cargo.toml の **直前または直後のコメント** で対象ターゲット (例: 「`thumbv6m-none-eabi` で `AtomicU64` を fallback 提供するため」) または用途 (例: 「`heapless` の portable-atomic 互換性確保のため」) が一文で説明されている
  - `docs/plan/YYYY-MM-DD-<topic>-evaluation.md` の評価レポートへの参照がコメントに含まれている
- **AND** 「歴史的な理由」「過去の change の名残」だけが理由で feature が残存している状態は許されない
- **AND** 評価レポートが存在する場合は当該レポート内で「維持」の根拠 (該当ターゲット、該当 atomic 幅、代替不可の理由) が示されている
