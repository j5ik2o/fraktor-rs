## Why

直前 merge された change `drop-actor-core-critical-section-dep`（PR #1605, #1606）で `tick_feed.rs` の直接 use を撤去し `actor-core/Cargo.toml` の `critical-section` を `optional = true` 化したが、`actor-core/test-support` feature には依然として **3 責務（A: impl provider 提供 / B: テスト用 API 公開 / C: 内部 API の `pub` 格上げ）** が混在している。

本 change は **責務 A の退役** を目的とする。`critical-section` クレートの本来の作法（ライブラリは依存宣言のみ、impl 選択はバイナリ側の責任）に従い、各バイナリ単位（test/bench/showcase）が `critical-section = { features = ["std"] }` を直接書く形に移行する。これにより `test-support` feature 自体は責務 B/C のみを抱える状態となり、最終的な feature 退役（別 change）への足場ができる。

## What Changes

- **BREAKING（ワークスペース内ダウンストリーム + 同パターン外部利用者）**: `actor-core/test-support` 経由で `critical-section/std` impl が自動配給されなくなる。fraktor-rs ワークスペース内の 5 クレート + showcase で `critical-section = { features = ["std"] }` の追加宣言が必要。fraktor-rs を library として使う外部利用者（`actor-core/test-support` を有効化していたケース）も同様の追加宣言が必要（ただし fraktor-rs はリリース前開発フェーズのため実質影響は workspace 内に限定）
- `actor-core/Cargo.toml:19` の `test-support = ["dep:critical-section", "critical-section/std"]` を `test-support = []` に変更
- `actor-core/Cargo.toml:24` の `critical-section = { workspace = true, default-features = false, optional = true }` を完全削除
- 各ダウンストリームクレートの修正は design Decision 3 を参照（最小追加・既存非破壊）
- `remote-core/Cargo.toml:17` の幽霊定義 `test-support = []` を削除
- `actor-lock-construction-governance` spec の Requirement B（前回 change で追加）の例外条項「`optional = true` かつ feature gated な impl provider 用エントリ」を MODIFIED で削除（本 change で例外発動箇所が消えるため）

## Capabilities

### New Capabilities

なし。

### Modified Capabilities

- `actor-lock-construction-governance`: Requirement B の例外条項を削除する MODIFIED。`optional + feature gated impl provider` の例外を撤去し、`actor-*` の `Cargo.toml` は「primitive lock crate を `[dependencies]` に直接宣言してはならない（推移的依存のみ許可）」というシンプルな規約に整理する

## Impact

### 影響を受けるコード

- `modules/actor-core/Cargo.toml`: `test-support` から impl 関連削除、`[dependencies]` の `critical-section` 完全削除
- `modules/cluster-core/Cargo.toml`: dev-deps に `critical-section/std` 追加
- `modules/cluster-adaptor-std/Cargo.toml`: dev-deps に `critical-section/std` 追加
- `modules/remote-adaptor-std/Cargo.toml`: dev-deps に `critical-section/std` 追加
- `modules/stream-core/Cargo.toml`: dev-deps に `critical-section/std` 追加
- `modules/stream-adaptor-std/Cargo.toml`: dev-deps に `critical-section/std` 追加
- `modules/remote-core/Cargo.toml:17`: 幽霊定義削除
- `showcases/std/Cargo.toml`: `[dependencies]` に `critical-section/std` 追加
- `openspec/specs/actor-lock-construction-governance/spec.md`: Requirement B の例外条項を MODIFIED で削除

### 影響を受けない範囲

- `modules/actor-adaptor-std/Cargo.toml`: 既に dev-deps で `critical-section = { features = ["std"] }` 直接記述済み（`:38`）
- `modules/persistence-core/Cargo.toml`: 既に dev-deps で `critical-section = { features = ["std"] }` 直接記述済み（`:28`）
- `modules/actor-core/Cargo.toml` の `[dev-dependencies]` の `critical-section = { workspace = true, features = ["std"] }`（`:42`）: actor-core 自身のテスト用、維持
- `actor-core/Cargo.toml` の 8 個の `required-features = ["test-support"]` integration test 設定: `test-support` feature 自体は維持されるため不変
- `tick_feed.rs` の `SharedLock + DefaultMutex` 実装（前回 change で完成）
- `utils-core/Cargo.toml` の `critical-section` 直接依存: `utils-core` は backend 実装層であり Requirement A/B の例外として許可される（actor-* ではない）

### 依存関係

- `actor-core` の `[dependencies]` から `critical-section` クレートが完全に消える（`portable-atomic` 経由 transitive のみ残る）
- 各バイナリ（test/bench/showcase）が impl provider 選択の責任を持つ標準的構造になる

### リスク

- **dev-dependencies 経由の test 動作確認**: 各クレートで `cargo test` が通ることを確認
- **showcase の動作確認**: `showcases/std` の実行が通ること
- **移行漏れ**: ダウンストリーム 5 クレート + showcase の修正で 1 つでも漏れると、対象クレートの `cargo test` または `cargo build` が `undefined reference to _critical_section_1_0_acquire` でリンク失敗する
- **責務 B/C は依然 `test-support` feature に残る**: 本 change のスコープは A のみ。`test-support` feature 自体の退役は別 change

### 後続 change（hand-off）

- 責務 B（`TestTickDriver`、`new_empty` 等の API 公開）の `actor-adaptor-std` への移動 / 専用 crate 切り出し
- 責務 C（内部 API の `pub` 格上げ）の `test-helpers` crate 化または `#[doc(hidden)]` 化
- 責務 B/C 完全退役後、`test-support` feature 自体の削除
