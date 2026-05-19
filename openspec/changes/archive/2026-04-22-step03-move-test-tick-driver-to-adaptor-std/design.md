## Context

### 現状の TestTickDriver 配置

`fraktor-actor-core-rs` は no_std クレートとして設計されているが、`test-support` feature 配下に以下の **std 依存コンポーネント** が同居している:

- `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/test_tick_driver.rs`（118 行）
  - `std::thread::{self, JoinHandle}` と `thread::sleep` を使って `TickFeed` に tick を供給する `TestTickDriver`
  - `TestTickDriverStopper` が join handle を保持してシャットダウン時に `join()` を呼ぶ
  - `#[cfg(any(test, feature = "test-support"))]` で gate
- `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver.rs`
  - `#[cfg(any(test, feature = "test-support"))] mod test_tick_driver;` と `pub use test_tick_driver::TestTickDriver;`
- `modules/actor-core/src/core/kernel/system/base.rs:72, 83`
  - `ActorSystem::new_empty()` と `ActorSystem::new_empty_with<F>()` が **`TestTickDriver::default()` を直接参照**
  - これらも `#[cfg(any(test, feature = "test-support"))]` で gate

### 下流の利用状況（本 change 起案時 Grep）

`TestTickDriver` は workspace 全体で **51 ファイル・215 箇所** で使われている:

- actor-core 内部のインラインテスト（`src/**/tests.rs`）: 20+ ファイル、100+ 箇所（`crate::core::kernel::actor::scheduler::tick_driver::TestTickDriver` パス参照）
- actor-core 統合テスト（`tests/*.rs`）: 8 ファイル（`use fraktor_actor_core_rs::...::TestTickDriver` 参照）
- 他クレート（actor-adaptor-std、cluster-core、stream-core、stream-adaptor-std、persistence-core）のテスト: 23 ファイル
- プロダクションコードでの参照は `actor-core` の `#[cfg(any(test, feature = "test-support"))]` ゲート配下のみ（外部 production caller ゼロ）

### actor-adaptor-std の受け皿

`modules/actor-adaptor-std/src/std/tick_driver/` には既に `StdTickDriver`（`std::thread` 利用）、`TokioTickDriver`（tokio runtime 依存）が存在する。`TestTickDriver` も同階層に配置するのが構造的に自然。`actor-adaptor-std` は既に `test-support` feature を持ち、`fraktor-actor-core-rs/test-support` を transitive に有効化している。

### 構造的制約: `new_empty` / `new_empty_with` との不可分性

`ActorSystem::new_empty_with<F>` は本体内で `TestTickDriver::default()` を直接参照している（`base.rs:86-88`）:

```rust
use crate::core::kernel::actor::scheduler::tick_driver::TestTickDriver;
let config = ActorSystemConfig::new(TestTickDriver::default());
```

proposal の当初案では `new_empty*` の移設を step04 に分割していたが、**TestTickDriver だけを先に移動すると `new_empty*` が定義先を失う**。さらに `actor-core` の prod lib（`feature = "test-support"` ゲートが有効な時は prod ビルド相当）から `actor-adaptor-std` を参照すると循環依存になる（`actor-adaptor-std` → `actor-core` が prod 依存）。

このため本 change は proposal を拡張し、`TestTickDriver` と合わせて `new_empty` / `new_empty_with` も同時に `actor-adaptor-std` 側へ移設する。

### Cargo の dev-cycle 許容について

Cargo は `[dev-dependencies]` 経由の循環依存を許容する（prod 依存は cycle 禁止だが dev 依存は例外）。`actor-core` の `[dev-dependencies]` に `fraktor-actor-adaptor-std-rs = { ..., features = ["test-support"] }` を追加することで、`actor-core` の `#[cfg(test)]` インラインテストと統合テストの両方から `actor-adaptor-std::...::TestTickDriver` を利用できる。

## Goals / Non-Goals

**Goals:**
- `TestTickDriver` を `actor-adaptor-std/src/std/tick_driver/test_tick_driver.rs` に移設し、既存 `StdTickDriver` / `TokioTickDriver` と同階層で統一的に管理
- `ActorSystem::new_empty` / `new_empty_with` を `actor-adaptor-std` 側の自由関数 `new_empty_actor_system` / `new_empty_actor_system_with<F>` に移設
- `actor-core` の `test-support` feature から `TestTickDriver` 系の公開シンボルを完全除去
- 下流 51 ファイルの import path を機械的に更新
- `actor-core` の `[dev-dependencies]` に `actor-adaptor-std` を追加し、Cargo dev-cycle を活用
- 新規 capability `actor-test-driver-placement` を ADDED し、std 依存テストヘルパの配置原則を spec 化

**Non-Goals:**
- `new_empty*` 以外の test-support 公開 API（`MockActorRef`、`TestProbe` 等）の移設は step04 で対応
- 独立した `fraktor-actor-test-rs` crate の新設は step04 のスコープ（本 change では `actor-adaptor-std` を受け皿にする）
- `test-support` feature 自体の削除は step06
- 内部 API 可視性格上げ（`Behavior::handle_message` 等）の解消は step05
- `actor-adaptor-std` の `test-support` feature 設計見直し（現状の構造を拡張する最小差分にとどめる）

## Decisions

### Decision 1: `TestTickDriver` の移設先は `actor-adaptor-std/src/std/tick_driver/test_tick_driver.rs`

**選択**: 既存 `StdTickDriver`（`std_tick_driver.rs`）/ `TokioTickDriver`（`tokio_tick_driver.rs`）と同階層に `test_tick_driver.rs` を配置する。`actor-adaptor-std/src/std/tick_driver.rs`（module file）に以下を追加（既存 `#[cfg(feature = "tokio-executor")] mod tokio_tick_driver;` と同じパターン）:

```rust
#[cfg(feature = "test-support")]
mod test_tick_driver;
#[cfg(feature = "test-support")]
pub use test_tick_driver::TestTickDriver;
```

**根拠**:
- 既存の 2 つの std 系 TickDriver 実装と完全に対応する配置で、発見性・命名・構造の一貫性が保たれる
- `TestTickDriver` は `std::thread` と `thread::sleep` に依存しているため、既存 `StdTickDriver` と同じ std 依存性プロファイル
- `actor-adaptor-std` の `test-support` feature を活用した gate が自然

**代替案と却下理由**:
- 案 A: `actor-adaptor-std/src/std/test_support/` のような専用ディレクトリを新設 → 既存 `tick_driver/` module のパターンと不整合、発見性低下
- 案 B: step04 で新設予定の `fraktor-actor-test-rs` crate に直接置く → step04 の範囲を step03 に前倒しすることになり、change の独立性が失われる
- 案 C: `actor-core` に残して公開 re-export だけ変える → 移設になっていない、test-support feature からの切り離し目標を達成しない

### Decision 2: `new_empty` / `new_empty_with` も同時移設し、自由関数として再実装

**選択**: proposal 原案の Non-Goals を修正し、`ActorSystem::new_empty` と `ActorSystem::new_empty_with<F>` を同時に `actor-adaptor-std` へ移設する。移設先は `modules/actor-adaptor-std/src/std/system/` 配下（module を新設）の自由関数として:

```rust
// actor-adaptor-std/src/std/system/empty_system.rs (新規)
pub fn new_empty_actor_system() -> ActorSystem {
  new_empty_actor_system_with(|config| config)
}

pub fn new_empty_actor_system_with<F>(configure: F) -> ActorSystem
where
  F: FnOnce(ActorSystemConfig) -> ActorSystemConfig,
{
  let config = ActorSystemConfig::new(TestTickDriver::default());
  let config = configure(config);
  let state = match SystemState::build_from_owned_config(config) {
    Ok(state) => state,
    Err(error) => panic!("test-support config failed to build in new_empty_actor_system_with: {error:?}"),
  };
  let system = ActorSystem::from_state(SystemStateShared::new(state));
  system.state.mark_root_started();  // pub(crate) の場合は公開 API 経由に変更必要
  system
}
```

`actor-core::ActorSystem` の `new_empty` / `new_empty_with` メソッドは削除する。

**根拠**:
- `new_empty*` は内部で `TestTickDriver::default()` を参照しており、`TestTickDriver` が `actor-adaptor-std` に移った時点で `actor-core` の lib 層（feature = "test-support" 有効時は prod 相当）から参照できない（循環依存）
- 自由関数化することで `ActorSystem` 本体の API surface を汚さない（std 依存のテスト専用 API が `actor-core::ActorSystem` の method として露出しなくなる）
- 下流 caller は `system.new_empty()` 形式を呼んでいるケースはほぼ無く、多くは `ActorSystem::new_empty()` / `ActorSystem::new_empty_with(...)` という関連関数形式で呼んでいるため、`new_empty_actor_system(...)` への書き換えは機械的に可能

**代替案と却下理由**:
- 案 A: `ActorSystem` を extension trait で拡張（`NewEmptyExt` 等）→ trait import が必要になり caller 側が煩雑、Rust のイディオムとして trait-based extension は最終手段
- 案 B: `new_empty*` を step04 に残し、step03 では `TestTickDriver` 移動後に `new_empty*` を一時的に panic/stub 化 → 中間状態が壊れる、避けるべき
- 案 C: step03 の順序を step04 の後に変える → step04 の fraktor-actor-test-rs crate が actor-core::TestTickDriver を参照する形になり、step04 完了後にまた step03 で import 書き換えが発生する（二度手間）
- 案 D: proposal を厳守して `new_empty*` を触らない → TestTickDriver 移動と両立不可能（本 Context で詳述）

**実装詳細 — actor-core 側で公開する必要がある内部 API**:

`new_empty_actor_system_with` を actor-adaptor-std 側の自由関数として実装するには、現在 `actor-core` の `ActorSystem::new_empty_with` が参照している以下の内部 API が `pub` として見える必要がある。tasks 1.5 で実体を確認し、最小差分で対応する:

1. **`ActorSystem::state` field**: 現在 private の可能性（private field 経由で `state.mark_root_started()` を呼んでいる）→ field 直接アクセスを避けるため、`pub fn mark_root_started(&self)` のような **inherent method を `ActorSystem` に追加** する方向が有力（field 直接公開は carried-over の問題が出やすい）
2. **`SystemStateShared::mark_root_started`**: 現在の可視性を確認。`pub(crate)` なら `pub` 化が必要
3. **`ActorSystem::from_state`**: 既に `pub` と確認済み（base.rs で見える範囲）
4. **`SystemStateShared::new`**: 同じく `pub` と確認済み
5. **`SystemState::build_from_owned_config`**: 現在の可視性を確認。`pub` でなければ `pub` 化する

**代替案**: `ActorSystem::new_started_from_config(config: ActorSystemConfig) -> Result<Self, ...>` のような公開 constructor を `actor-core` 側に追加し、adaptor-std はそれを呼ぶだけにする。内部 API 公開を最小化する意味で有力な選択肢。tasks 実装フェーズで最終判断する。

### Decision 3: `actor-core` の `[dev-dependencies]` に `actor-adaptor-std` を追加（Cargo dev-cycle）

**選択**: `modules/actor-core/Cargo.toml` の `[dev-dependencies]` に以下を追加:

```toml
fraktor-actor-adaptor-std-rs = { workspace = true, features = ["test-support"] }
```

**根拠**:
- Cargo は prod 依存の循環は禁止するが、`[dev-dependencies]` 経由の循環（dev-cycle）は許容する
- `actor-core` のインラインテスト（`src/**/tests.rs` 内の `#[cfg(test)] mod tests;`）および統合テスト（`tests/*.rs`）から `actor-adaptor-std::TestTickDriver` を利用可能
- `actor-adaptor-std` → `actor-core`（prod 依存、変わらず）
- `actor-core`（test 時のみ）→ `actor-adaptor-std`（dev 依存、新規）

**代替案と却下理由**:
- 案 A: `actor-core` から TestTickDriver 依存を完全に取り除く（インラインテストを書き換え） → 20+ ファイルのテスト再設計が必要、scope が爆発
- 案 B: `actor-core` にテスト専用の軽量 stub（`InlineTestTickDriver` 等）を追加 → TestTickDriver と stub の 2 重管理、重複
- 案 C: Cargo の dev-cycle を使わず、テストを別 crate に切り出す → step04 の fraktor-actor-test-rs と役割が被る

**Risks**:
- dev-cycle は Cargo 公式に許容されているが、一部のツール（IDE、rust-analyzer 等）が混乱する可能性がある → tasks 段階で `cargo metadata -p fraktor-actor-core-rs` と `cargo test -p fraktor-actor-core-rs --all-features` の挙動を確認し、rust-analyzer 側の挙動は IDE 上で手動確認（症状が出た場合のみ workaround 検討）

### Decision 4: spec delta は新規 capability `actor-test-driver-placement` を ADDED

**選択**: 新規 capability として `actor-test-driver-placement` を追加。`openspec/specs/actor-test-driver-placement/spec.md` を新設し、以下の Requirement を持つ:

- Requirement: **std 依存のテストドライバおよびテストヘルパ** は actor-adaptor-std 側に配置されなければならない

Scenario は以下 4 つ（specs/actor-test-driver-placement/spec.md で詳細記述）:
- `TestTickDriver` の定義・再エクスポートが `actor-core` 側に存在しないこと
- std 依存のテストコンストラクタ（`new_empty*` 等）が `actor-core::ActorSystem` の method から削除されていること
- `actor-core` の `[dev-dependencies]` に `actor-adaptor-std = { features = ["test-support"] }` が宣言され、`[dependencies]` には含まれないこと（dev-cycle のみ許容）
- 下流クレートが `fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver` 形式で import していること

**根拠**:
- 「std 依存コンポーネントは actor-adaptor-std 側」というドメインルールは governance 価値があり、他クレート（将来の test-support 類似案件）でも援用可能
- 既存 `actor-lock-construction-governance` spec に Scenario 追加する案もあるが、そちらは lock 構築のガバナンスであり、テストドライバ配置は別ドメイン
- capability を分けることで、step04 以降の関連 change が capability を ADDED / MODIFIED する形で拡張できる

**代替案と却下理由**:
- 案 A: 既存 `actor-lock-construction-governance` に Scenario 追加 → ドメインが異なる、無理に同居させるとルール集が散漫化
- 案 B: spec delta 最小で既存 capability に Scenario のみ追加 → OpenSpec validation は通るがドメイン明確性を欠く
- 案 C: spec delta を作らない → OpenSpec strict validation が通らない

### Decision 5: 移設 API 名は `new_empty_actor_system` / `new_empty_actor_system_with`

**選択**: `ActorSystem::new_empty` → 自由関数 `new_empty_actor_system`、`ActorSystem::new_empty_with` → `new_empty_actor_system_with<F>` とする。`fraktor_actor_adaptor_std_rs::std::system::{new_empty_actor_system, new_empty_actor_system_with}` として公開。

**根拠**:
- 関連関数（`::new_empty`）を自由関数化する場合、関数名だけでは「何を作るか」が不明確なため、対象型名を含めて `new_empty_actor_system` とする
- Rust の慣習に合う（`std::fs::read_to_string` のような「module:: verb_object」形式）

**代替案と却下理由**:
- 案 A: `empty_actor_system` / `empty_actor_system_with` → `new_` プレフィックスを省略すると constructor であることが伝わりにくい
- 案 B: `ActorSystem::test_empty()` のように型のメソッドとして残す → `TestTickDriver` 依存が `actor-core::ActorSystem` に残ってしまい、結局 `actor-core` から `actor-adaptor-std` 参照が必要（本末転倒）
- 案 C: `test_actor_system()` → 「テスト専用である」が読み取れるが「空の」意味が抜け、`new_empty_actor_system` ほど明示的でない

## Risks / Trade-offs

- **[Risk] Cargo dev-cycle が rust-analyzer / IDE で誤検出される可能性** → Mitigation: 実装後 `cargo check --workspace` / `cargo test --workspace` が通ることを CI で確認。rust-analyzer 側の問題は workaround として `rust-analyzer: cargo: allTargets` 等の設定があるが、workspace 全体としての動作影響は軽微。tasks に挙動確認を含める
- **[Risk] 51 ファイルの import path 更新で書き換え漏れ** → Mitigation: `Grep` で一括検索し、置換後にも再 Grep で 0 hits 確認。CI の全テスト通過で担保
- **[Risk] `mark_root_started` 等の `pub(crate)` な内部 API が actor-adaptor-std から見えず、移設先で panic を起こす** → Mitigation: 実装時に該当 API を公開（`pub`）化するか、`ActorSystem::new_from_state_and_start` のような公開 constructor を追加する。本 change の design で事前に列挙し、tasks で個別対処
- **[Risk] step04 で `fraktor-actor-test-rs` crate を新設する際、`actor-adaptor-std` の `test-support` との責務分離が不明瞭になる** → Mitigation: step04 の design で「actor-adaptor-std は std 依存のランタイム補助、actor-test は std 非依存のテストヘルパ（mock、probe 等）」のような住み分けを明確化する。本 change では `TestTickDriver` / `new_empty*` のような std 依存ヘルパに絞って adaptor-std 側に置く
- **[Trade-off] proposal の当初 scope（TestTickDriver のみ）からの拡大** → 受容: 構造的依存関係により分離不能、Decision 2 で詳述

## Migration Plan

1. **Phase 1: actor-adaptor-std 側の受け皿整備**
   - `modules/actor-adaptor-std/src/std/tick_driver/test_tick_driver.rs` 新規作成（actor-core の該当ファイルから移植）
   - `modules/actor-adaptor-std/src/std/tick_driver.rs` に `#[cfg(feature = "test-support")] mod test_tick_driver;` と `#[cfg(feature = "test-support")] pub use test_tick_driver::TestTickDriver;` を追加
   - `modules/actor-adaptor-std/src/std/system/empty_system.rs` 新規作成（`new_empty_actor_system` / `new_empty_actor_system_with` 実装）
   - `modules/actor-adaptor-std/src/std/system.rs` 新規作成（module file、`#[cfg(feature = "test-support")] mod empty_system;` と `#[cfg(feature = "test-support")] pub use empty_system::{new_empty_actor_system, new_empty_actor_system_with};` を明示的に記述、glob export は避ける）
   - `modules/actor-adaptor-std/src/std.rs` に `#[cfg(feature = "test-support")] pub mod system;` を追加
2. **Phase 2: actor-core 側の削除と必要最小限の API 公開**
   - `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/test_tick_driver.rs` を削除
   - `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver.rs` の `#[cfg(any(test, feature = "test-support"))] mod test_tick_driver;` と対応する `pub use` を削除
   - `modules/actor-core/src/core/kernel/system/base.rs` の `new_empty` / `new_empty_with` 両 method を削除（他の `#[cfg]` ゲート要素は touch しない）
   - adaptor-std の `new_empty_actor_system_with` が必要とする internal API を公開する（Decision 2 実装詳細に従い、案 A: 5 つの field / method を個別に `pub` 化 / 案 B: `ActorSystem::new_started_from_config(config)` のような公開 constructor を追加、のどちらか）
3. **Phase 3: actor-core Cargo.toml 更新**
   - `[dev-dependencies]` に `fraktor-actor-adaptor-std-rs = { workspace = true, features = ["test-support"] }` 追加
4. **Phase 4: 下流 import path 更新**
   - `Grep` で対象全ファイルを特定（起案時集計: TestTickDriver 51 ファイル / 215 箇所、`new_empty*` 系 caller は tasks 1.2 で集計）
   - インラインテスト（`src/**/tests.rs`）: `use crate::core::...::TestTickDriver;` → `use fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver;`
   - 統合テスト（`tests/*.rs`）: `use fraktor_actor_core_rs::...::TestTickDriver;` → 同上
   - `ActorSystem::new_empty()` / `ActorSystem::new_empty_with(...)` → `new_empty_actor_system()` / `new_empty_actor_system_with(...)` に置換、同時に `use fraktor_actor_adaptor_std_rs::std::system::{new_empty_actor_system, new_empty_actor_system_with};` の import を追加
5. **Phase 5: ビルド・テスト検証**
   - `cargo build --workspace --no-default-features` と `cargo build --workspace --all-features`
   - `cargo test --workspace`
   - `cargo clippy --workspace`
6. **Phase 6: spec 整合**
   - 新規 `specs/actor-test-driver-placement/spec.md` 作成（change delta に基づく）
   - `openspec validate --strict` 通過確認
7. **Phase 7: 全体 CI 確認**
   - `./scripts/ci-check.sh ai all` で workspace 全体 green
8. **Phase 8: ドキュメント更新**
   - `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 1 責務 B の進捗を更新（責務 B-1 完了、残 B-2 は step04）

ロールバックは git revert で完結する。

## Open Questions

- `actor-adaptor-std` の `test-support` feature は現状 `fraktor-actor-core-rs/test-support` を transitive に有効化している。TestTickDriver / new_empty* 移設後も actor-core 側 test-support feature は step04 以降まで残るため、transitive 有効化は維持する（本 change で touch しない）

## 実装後の補足: Cargo dev-cycle の制約と対応

実装段階で **dev-cycle 経由で actor-core の inline test から actor-adaptor-std::TestTickDriver を使うアプローチが Cargo の根本的制約に当たることが判明**した。Cargo は `[dev-dependencies]` 経由の cycle を許容するものの、 inline test ビルドにおいては同一クレート（`fraktor_actor_core_rs`）が "lib 直接" と "actor-adaptor-std 経由の transitive" の二バージョンとして compiler に見え、型不一致 (`expected ActorSystem, found a different ActorSystem`) を起こす。これは Rust/Cargo の仕様であり回避不能。

このため実装では以下の二段構成を採った:

**1. `actor-core` 内部の inline test 専用ヘルパ（test-only, 非公開）**:

- `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/tests/test_tick_driver.rs` に `pub(crate) struct TestTickDriver` を保持
- 親 module は `pub(crate) mod tests;`（`#[cfg(test)]` ではなく `#![cfg(test)]` を tests.rs 側で使う Pekko 風 idiom）。これにより dylint `tests_location_lint` も pass
- `ActorSystem::new_empty()` / `new_empty_with()` も `modules/actor-core/src/core/kernel/system/base/tests.rs` 内に `impl ActorSystem { pub(crate) fn ... }` 形式で配置
- `TypedActorSystem::<M>::new_empty()` も同様に `modules/actor-core/src/core/typed/system/tests.rs` 内に配置
- これらは `pub(crate)` のため crate 内 `#[cfg(test)]` コードからのみアクセス可能。外部からは見えない

**2. `actor-adaptor-std` 側の公開 API**:

- `modules/actor-adaptor-std/src/std/tick_driver/test_tick_driver.rs` の `pub struct TestTickDriver`
- `modules/actor-adaptor-std/src/std/system/empty_system.rs` の `new_empty_actor_system()` / `new_empty_actor_system_with()` / `new_empty_typed_actor_system::<M>()` 自由関数
- すべて `#[cfg(feature = "test-support")]` ゲート

**3. caller の選択基準**:

| caller | 利用するヘルパ |
|---|---|
| `actor-core` の inline test (`src/**/tests.rs`) | actor-core 内部版（`crate::...::tick_driver::tests::TestTickDriver` / `ActorSystem::new_empty()`） |
| `actor-core` の integration test (`tests/*.rs`) | actor-adaptor-std 公開版（`fraktor_actor_adaptor_std_rs::std::tick_driver::TestTickDriver` / `new_empty_actor_system()`） |
| 下流 crate (cluster-core / stream-core / persistence-core / actor-adaptor-std 自身) | actor-adaptor-std 公開版 |

これにより:

- **責務 B-1 の本質的目標は達成**: `actor-core` の `test-support` feature の **公開 API には TestTickDriver / new_empty* が含まれない**（公開されない `pub(crate)` のみ）
- 外部 caller は actor-adaptor-std の公開 API のみを利用するため、no_std 原則を犯す std 依存ヘルパが actor-core から外部に漏れることはない
- 内部 inline test だけは依然として std を使うが、これは既存の `extern crate std;` + `#[cfg(test)]` の慣行と整合的（Rust の test profile は host で std 利用可）

`actor-core/Cargo.toml` の `[dev-dependencies]` に追加した `fraktor-actor-adaptor-std-rs` は、actor-core の **integration test** が actor-adaptor-std の公開 API を使うために必須（inline test 用ではない）。
- `new_empty_actor_system` 系関数の rustdoc 例示コードで `TestTickDriver::default()` を直接見せるかどうか → 実装フェーズで caller 体験を見て最小限の例示にとどめる
