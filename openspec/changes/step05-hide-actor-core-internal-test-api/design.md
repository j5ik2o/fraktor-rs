## Context

step03 完了時点で `actor-core/test-support` feature 配下に残っている `#[cfg(any(test, feature = "test-support"))]` ゲートを **全数調査** した結果、以下が判明した。

### 全数棚卸し (14 箇所、シンボル単位 11 件)

| # | Symbol | File | gate 種別 | 外部 caller (workspace) | 戦略候補 |
|---|---|---|---|---|---|
| 1 | `Behavior::handle_message` | `core/typed/behavior.rs:204` | pub-promote | 0 (actor-core inline test のみ) | A |
| 2 | `Behavior::handle_start` | `core/typed/behavior.rs:243` | pub-promote | 0 (actor-core inline test のみ) | A |
| 3 | `Behavior::handle_signal` | `core/typed/behavior.rs:274` | pub-promote | 0 (actor-core inline test + actor-core production intra-crate) | A |
| 4 | `TypedActorContext::from_untyped` | `core/typed/actor/actor_context.rs:50` | pub-promote | 0 (actor-core inline test のみ) | A |
| 5 | `state::booting_state` mod | `core/kernel/system/state.rs:21` | mod-existence toggle | 0 (actor-core inline test + intra-crate from running_state) | A |
| 6 | `state::running_state` mod | `core/kernel/system/state.rs:23` | mod-existence toggle | 0 (booting_state からのみ参照、production 経路なし) | A |
| 7 | `SystemState::register_guardian_pid` | `core/kernel/system/state/system_state.rs:500` | method-existence toggle (両分岐 pub(crate)) | 0 (booting_state.rs:19 + actor-core inline test) | A |
| 8 | `SystemStateShared::register_guardian_pid` | `core/kernel/system/state/system_state_shared.rs:430` | method-existence toggle (両分岐 pub(crate)) | 0 (actor-core inline test のみ) | A |
| 9 | `ActorRef::new_with_builtin_lock` | `core/kernel/actor/actor_ref/base.rs:90` | pub-promote (`new_with_builtin_lock_impl` も同梱) | 0 (actor-core inline test のみ) | A |
| 10 | `SchedulerRunner::manual` | `core/kernel/actor/scheduler/scheduler_runner.rs:39` | pub-promote | 0 (actor-core inline test のみ) | A |
| 11 | `TickDriverBootstrap` struct + `provision` メソッド | `core/kernel/actor/scheduler/tick_driver/bootstrap.rs:17,28` | pub-promote (両分岐 pub/pub(crate) 切替) + `tick_driver.rs:25` の re-export | 0 (system_state.rs:183/258 production intra-crate + actor-core inline test + actor-core integration test → adaptor-std 経由パス未使用) | A |

> `BootingSystemState`、`RunningSystemState` は `pub(crate)` だが production code path から呼ばれていない (実体は test 用 scaffold)。これらを残すかは Decision 4 で扱う。

### 重要な制約

- **dev-cycle 知見 (step03)**: actor-core inline test (`src/**/tests.rs`) は同一クレート二バージョン問題により外部 crate 由来のヘルパを利用できない。`pub(crate)` 化した場合は inline test がそのまま利用継続できるが、もし今後 `pub` 公開が必要なら **inline test を統合 test (`tests/*.rs`) に移行する** 必要がある
- **lint 制約**: `tests_location_lint` により production 本体ファイルへの `#[cfg(test)]` 直接記述は禁止。test-only コードは `<module>/tests.rs` (file-level `#![cfg(test)]`) に置く必要がある (step03 で確立した pekko 風 idiom)

## Goals / Non-Goals

**Goals:**

- `modules/actor-core/src/` 配下で `#[cfg(any(test, feature = "test-support"))]` 経由で **可視性を拡大している箇所を 0 件** にする
- 全 11 シンボルを `pub(crate)` (or `#[cfg(test)]` only) に縮小し、`feature = "test-support"` ゲートを削除
- step06 (feature 削除) が `actor-core/test-support = []` (空 feature) または完全削除を機械的に行えるよう道筋をつける

**Non-Goals:**

- `actor-adaptor-std::TestTickDriver` / `new_empty_actor_system*` の再配置 (step03 確定済み)
- step03 で導入した actor-core 内部 `pub(crate)` 限定 dev-cycle workaround helper の撤去 (本 change の範囲外、別 change で扱う)
- `actor-core` の public API surface の再設計 (本 change は visibility 縮小のみ、新規 public API の追加はしない)
- 純粋な `#[cfg(test)]` (test-support feature を含まない) ゲートの整理 (これは test 専用で外部影響なし、保持)

## Decisions

### Decision 1: 全 11 シンボル A 戦略 (pub(crate) 化) 一律適用

調査の結果、全シンボルで **外部 crate からの caller が 0 件** であることが確定した。proposal で挙げていた A/B/C 戦略のうち、本 change では **A 戦略 (pub(crate) 化) のみ採用** する。

**根拠**:

- B 戦略 (正式 public 化) は外部 caller がいて初めて意味を持つ。0 caller の API を新規 public 化するのは YAGNI 違反
- C 戦略 (inline test を統合 test 移行) は inline test が pub 化された API を使っている場合の選択肢。本 change では `pub(crate)` で十分なので適用外
- 全箇所一律 A にすることで判定ロジック・実装手順が単純化し、レビュー負担も減る

**Alternatives considered**:

- 戦略 B (一部 public 化): 例えば `Behavior::handle_*` を test-friendly な public API として整備する案。→ caller がいないので不要、将来必要になった時点で別 change で行う
- 戦略 C (inline test 大量移行): inline test を全部統合 test に移すと test ビルド時間が伸びる + 多数の moving parts。本 change は visibility 縮小に集中し、test 構造改革は別 change

### Decision 2: cfg gate の削除パターンを統一

各箇所で `#[cfg(any(test, feature = "test-support"))]` ゲートを削除する具体パターンを統一:

#### パターン 1: pub-promote (両分岐がある場合) → 単純削除

```rust
// Before
#[cfg(any(test, feature = "test-support"))]
pub fn foo(...) -> ... { Self::foo_impl(...) }
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) fn foo(...) -> ... { Self::foo_impl(...) }

// After
pub(crate) fn foo(...) -> ... { Self::foo_impl(...) }
```

該当: `Behavior::handle_message` / `handle_start` / `handle_signal`、`TypedActorContext::from_untyped`、`ActorRef::new_with_builtin_lock` (および `new_with_builtin_lock_impl`)、`SchedulerRunner::manual`、`TickDriverBootstrap` struct + `provision`、`tick_driver.rs` の `pub use TickDriverBootstrap`。

#### パターン 2: method-existence toggle (両分岐 pub(crate)) → ゲート単純削除

```rust
// Before
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn register_guardian_pid(&mut self, kind: GuardianKind, pid: Pid) { ... }

// After
pub(crate) fn register_guardian_pid(&mut self, kind: GuardianKind, pid: Pid) { ... }
```

該当: `SystemState::register_guardian_pid`、`SystemStateShared::register_guardian_pid`。

production code (`booting_state.rs:19`) からも呼ばれているため、常に存在させる。

#### パターン 3: mod-existence toggle → 単純削除 (booting_state / running_state)

```rust
// Before
#[cfg(any(test, feature = "test-support"))]
mod booting_state;
#[cfg(any(test, feature = "test-support"))]
mod running_state;

// After
mod booting_state;
mod running_state;
```

これら 2 モジュールは `pub(crate)` 型を提供するが、production code path から実際には呼ばれない (Decision 4 参照)。本 change では削除せず、ゲートだけ外して常に存在させる。死んだ test scaffold としての扱いは別 change で再評価する。

### Decision 3: TickDriverBootstrap re-export の削除

`tick_driver.rs:25-28` の以下:

```rust
#[cfg(any(test, feature = "test-support"))]
pub use bootstrap::TickDriverBootstrap;
#[cfg(not(any(test, feature = "test-support")))]
pub(crate) use bootstrap::TickDriverBootstrap;
```

を以下に置換:

```rust
pub(crate) use bootstrap::TickDriverBootstrap;
```

`bootstrap.rs` 側の struct 定義 + impl も同様に `pub(crate)` 一本化する。

### Decision 4: BootingSystemState / RunningSystemState の扱い

調査で「production code path から呼ばれていない」と判明したが、本 change ではあえて削除せずゲート外しのみ行う。

**根拠**:

- 本 change のスコープは「`feature = "test-support"` 由来の visibility 拡大の全数撤廃」であり、コード本体の dead-code 撤去は別問題
- 仮に test scaffold として有用 (例: 将来 SystemState の lifecycle を可視化する用途) なら残すべき
- もし不要と判断したら別 change で `safe_delete_symbol` 経由で除去する

### Decision 5: dev-cycle workaround helper との関係

step03 で actor-core 内部に `pub(crate)` 限定で残した:

- `tick_driver/tests/test_tick_driver.rs` の `pub(crate) struct TestTickDriver`
- `system/base/tests.rs` の `impl ActorSystem { pub(crate) fn new_empty / new_empty_with }`
- `typed/system/tests.rs` の `impl TypedActorSystem<M> { pub(crate) fn new_empty }`

これらは本 change のスコープ **外**。理由:

- これらはすでに `pub(crate)` であり、`feature = "test-support"` ゲートも経由していない
- 撤去するには inline test を統合 test に移行する大規模リファクタが必要 (別 change)

## Risks / Trade-offs

- **[Risk] 大量箇所の一括変更でテストレグレッション**:
  - Mitigation: 1 シンボルずつ `pub(crate)` 化 → `cargo test -p fraktor-actor-core-rs --lib` でレグレッションを確認 → コミット、を繰り返す。一括 push しない
- **[Risk] downstream crate の test-support feature 使用箇所が見落とされている**:
  - Mitigation: 全 caller を再確認済み (caller 0 件)。念のため `cargo test --workspace --features test-support` と `cargo test --workspace` (feature 無し) の両方で test pass を確認
- **[Trade-off] proposal で挙げた B/C 戦略を結局使わなかった**:
  - 受容: 調査結果で B/C の出番がないと確定したため。将来 caller が現れたら再評価

## Migration Plan

1. **Phase 1: pub-promote 系のゲート削除 (シンボル単位、約 9 シンボル)**
   - `Behavior::handle_message` / `handle_start` / `handle_signal`
   - `TypedActorContext::from_untyped`
   - `ActorRef::new_with_builtin_lock` (`new_with_builtin_lock_impl` も)
   - `SchedulerRunner::manual`
   - `TickDriverBootstrap` struct + `provision` (および `tick_driver.rs` の re-export)
   - 各シンボルごとに `cargo test -p fraktor-actor-core-rs --lib` で確認
2. **Phase 2: method/mod existence toggle 系のゲート削除 (5 箇所)**
   - `SystemState::register_guardian_pid`
   - `SystemStateShared::register_guardian_pid`
   - `state::booting_state` mod 宣言
   - `state::running_state` mod 宣言
3. **Phase 3: workspace 全体の grep 確認**
   - `Grep "feature = \"test-support\""` で actor-core 配下 0 件確認
   - `cargo test --workspace` および `cargo test --workspace --features test-support` 両方 pass 確認
4. **Phase 4: spec delta 適用と CI**
   - `actor-test-driver-placement` capability に MODIFIED で「`feature = \"test-support\"` 経由の可視性拡大は actor-core では一切許容しない」Scenario を追加
   - `./scripts/ci-check.sh ai all` 緑確認
5. **Phase 5: ドキュメント更新**
   - `docs/plan/2026-04-21-actor-core-critical-section-followups.md` 残課題 1 責務 C を「解消済み」に更新
   - 責務 B-2 残も併記して「解消済み」に更新
6. **Phase 6: コミット + PR**
   - シンボルごとに小さくコミット (Phase 1-2 で約 14 commit)
   - PR 作成 → レビュー → マージ

ロールバックは git revert で完結する。

## Open Questions

- 本 change 完了後、`actor-core/test-support` feature は完全に空 (`[]`) になる想定。step06 で `[features]` セクションから削除する形で良いか? (= step06 の scope 確認)
- `BootingSystemState` / `RunningSystemState` の扱いを Decision 4 では「現状維持 (ゲート外しのみ)」としたが、production caller 0 件の test scaffold を残す価値があるかは別途レビュー候補
- inline test の統合 test 移行 (dev-cycle workaround helper の撤去) は本 change の Non-Goals としたが、別 change として開始する優先度は? (step08 以降に組み込むか、別途立てるか)
