# `portable-atomic/critical-section` feature 評価レポート

調査 change: `step07-evaluate-portable-atomic-critical-section-need`
作成日: 2026-04-22

## サマリー

`modules/actor-core/Cargo.toml:24` の以下の宣言を退役できるかを評価した:

```toml
portable-atomic = { workspace = true, default-features = false, features = ["critical-section"] }
```

**結論: 案 Y (現状維持) を強く推奨。**

CI が `thumbv8m.main-none-eabi` (32-bit ARM Cortex-M33、ARMv8-M Mainline) で `actor-core` を `cargo check` しており、production code 内に `AtomicU64` の利用が **8 ファイル** ある。`thumbv8m.main` は 64-bit atomic のハードウェアサポートを持たないため、`portable-atomic/critical-section` による emulated atomic が必須。退役した瞬間に no-std CI ジョブが破綻する。

## Phase 1: CI ターゲット棚卸し

### 調査対象

`.github/workflows/**.yml` および `scripts/ci-check.sh`。

### 抽出結果

| ターゲット triple | 利用箇所 | カテゴリ | AtomicU64 ハードサポート |
|------------------|---------|---------|-----------------------|
| ホスト (x86_64-unknown-linux-gnu / aarch64-apple-darwin など runner 依存) | `format`, `lint`, `test`, `docs`, `examples`, `no-std-host-*` | 64-bit | あり |
| `thumbv8m.main-none-eabi` | `scripts/ci-check.sh:1056-1067` `run_no_std()` の `no-std-thumb-utils` / `no-std-thumb-core` | 32-bit ARMv8-M Mainline (Cortex-M33 等) | **なし** (emulation 必須) |

### 補足: scripts/ci-check.sh:10 の THUMB_TARGETS 配列

```bash
THUMB_TARGETS=("thumbv6m-none-eabi" "thumbv8m.main-none-eabi")
```

- 両方とも 32-bit ARM、AtomicU64 ハードサポートなし
- 実行 `run_no_std()` 内で使われているのは `thumbv8m.main-none-eabi` のみ (line 1056)
- `thumbv6m-none-eabi` (Cortex-M0/M0+) は `run_embedded()` (line 1091〜) でコメントアウト中の future work
- `THUMB_TARGETS` 配列が存在する事実自体が「組み込み 32-bit を想定対象に含めている」という方針表明

### 結論 (Phase 1)

CI が現役で組み込み 32-bit (`thumbv8m.main`) で `actor-core` を check しており、ターゲットは AtomicU64 のハードサポートを持たない。`portable-atomic/critical-section` の emulated atomic が **CI 通過のため必須**。

## Phase 2: portable_atomic 利用 census

### 抽出方法

```bash
rg -n 'use portable_atomic' modules/ --glob '*.rs' --glob '!**/tests.rs' --glob '!**/tests/*.rs'
```

production code (test ファイル除く) のみを対象。

### actor-core 内 census

| ファイル | symbol | 要求幅 | 代替候補 (`core::sync::atomic`) | 備考 |
|---------|--------|-------|------------------------------|------|
| `core/kernel/actor/actor_cell.rs:18` | `AtomicBool` | 1-bit | `core::sync::atomic::AtomicBool` (全ターゲット OK) | 32-bit 安全 |
| `core/kernel/actor/actor_ref/base.rs:14` | `AtomicU64` | 64-bit | **代替不可** (32-bit ターゲットでは hardware なし) | thumbv8m で emulation 必須 |
| `core/kernel/actor/fsm/machine.rs:8` | `AtomicU64` | 64-bit | **代替不可** | 同上 |
| `core/kernel/actor/scheduler/tick_driver/tick_driver_trait.rs:6` | `AtomicU64` | 64-bit | **代替不可** | 同上 |
| `core/kernel/actor/scheduler/tick_driver/tick_feed.rs:14` | `AtomicU64` | 64-bit | **代替不可** | 同上 |
| `core/kernel/event/stream/actor_ref_subscriber.rs:6` | `AtomicU64` | 64-bit | **代替不可** | 同上 |
| `core/kernel/routing/random_routing_logic.rs:6` | `AtomicU64` | 64-bit | **代替不可** | 同上 |
| `core/kernel/system/state/system_state.rs:22` | `AtomicBool, AtomicU64` | 1-bit + 64-bit | AtomicBool は OK、`AtomicU64` 代替不可 | 同上 |
| `core/typed/dsl/routing/group_router.rs:7` | `AtomicU64` | 64-bit | **代替不可** | 同上 |
| `core/typed/dsl/routing/pool_router.rs:7` | `AtomicU64` | 64-bit | **代替不可** | 同上 |

集計:
- `AtomicU64` 利用ファイル数: **8 ファイル** (production)
- `AtomicBool` のみのファイル数: 1 (`actor_cell.rs`)
- `AtomicBool` + `AtomicU64`: 1 (`system_state.rs`)

### stream-core 内 census

| ファイル | symbol | 要求幅 | 代替候補 |
|---------|--------|-------|---------|
| `core/shape/port_id.rs:1` | `AtomicU64` | 64-bit | **代替不可** |
| `core/impl/materialization/stream_handle_id.rs:1` | `AtomicU64` | 64-bit | **代替不可** |

### utils-core 内 census

| ファイル | symbol | 種別 |
|---------|--------|------|
| `core/sync/arc_shared.rs:13` | `portable_atomic_util::Arc` | `Arc` 代替 (atomic primitive ではない) |
| `core/sync/weak_shared.rs:8` | `portable_atomic_util::Weak` | `Weak` 代替 (同上) |

`portable_atomic_util` は `portable-atomic-util` クレート由来で、`portable-atomic/critical-section` feature とは別。`portable-atomic-util` が `portable-atomic` を依存として引き込み、その `portable-atomic` の `critical-section` feature 有効化要否は別途評価が必要だが、`utils-core` 経由で actor-core にも transitively つながっている。

### 結論 (Phase 2)

- production code 内の `AtomicU64` 利用は **少なくとも 10 ファイル** (actor-core 8 + stream-core 2)
- いずれも 32-bit ARM (thumbv8m.main / thumbv6m) では hardware support なしで、`portable-atomic/critical-section` の emulated atomic 経由でのみビルド可能

## Phase 3: 影響度評価

Phase 1 と Phase 2 の結果から:

- CI が `thumbv8m.main-none-eabi` を対象にしている → 組み込み 32-bit ターゲットが対象内
- 利用箇所が 10 ファイルある → `AtomicU64` を必要とする箇所多数

**ケース分類: B (維持必須)**

`portable-atomic/critical-section` を退役すると:

1. `cargo check -p fraktor-actor-core-rs --target thumbv8m.main-none-eabi` が compile error
   - エラー例: `use portable_atomic::AtomicU64;` → portable-atomic の AtomicU64 が emulation なしでは 32-bit target で構築できない
2. `scripts/ci-check.sh no-std` が fail
3. CI ジョブ `Test (no_std)` が red

退役は **不可**。

## Phase 4: 利用実績調査

ケース B (維持必須) が確定したため Phase 4 は **スキップ**。実利用報告の有無に関係なく、CI 自体が組み込み 32-bit 対応を要求している事実が決定的。

## Decision Matrix

| 選択肢 | 内容 | Pros | Cons | step08 で採るべき場合 |
|--------|------|------|------|----------------------|
| **X: 退役** | `portable-atomic/critical-section` feature を削除し、利用箇所を `core::sync::atomic` に置換 | 依存縮小、`critical-section` 推移依存解消 | **CI 即破綻** (no-std-thumb-core が compile error)、組み込み 32-bit 対応が損なわれる | 採用不可 (CI が組み込み対応を要求している現状では成立しない) |
| **Y: 維持 ✓ (推奨)** | 現状維持。`portable-atomic/critical-section` feature を保持。Cargo.toml にコメントで根拠を残す | CI 通過、組み込み 32-bit 対応が維持される、step08 で扱う対象が消滅して Strategy B が論理的に閉じる | `critical-section` 推移依存が残る (が、これは組み込み対応の必然) | **本評価結果のケース B** |
| **Z: 条件付き維持** | actor-core に feature flag (例: `embedded-atomic-fallback`) を追加し、`portable-atomic/critical-section` を opt-in 化 | std/64-bit 系のデフォルトで `critical-section` 依存が消える | feature 設計コスト、CI で opt-in を毎回指定する保守コスト、誤用 (opt-in 忘れ) で組み込み運用者が壊れる | 「std/64-bit デフォルトで critical-section 依存を完全に絶ちたい」という強い動機があり、かつ CI / utils 側でも整合できる場合のみ。本 change のスコープ外 |

## Recommendation

**案 Y (現状維持) を採用**。理由:

1. CI が `thumbv8m.main-none-eabi` で `actor-core` を check している (scripts/ci-check.sh:1056)
2. production code に `AtomicU64` 利用が 10 ファイル存在する
3. 退役した瞬間に no-std CI が compile error で fail する
4. `THUMB_TARGETS` 配列の存在 (scripts/ci-check.sh:10) が「組み込み 32-bit を想定対象に含む」という方針表明として機能している

step08 (`step08-retire-portable-atomic-critical-section`) は **中止** すべき。中止理由: 本評価で「組み込み 32-bit 対応が CI 上必須であり、退役すると即破綻する」と確定したため。

ただし、本評価で判明した「歴史的根拠が Cargo.toml に残っていない」状態は問題なので、以下の最小限の改善は本 change で実施:

- `modules/actor-core/Cargo.toml:24` の `portable-atomic` 行に **直前コメント** で「`thumbv8m.main` 等の 32-bit 組み込みで `AtomicU64` を fallback 提供するため、`critical-section` feature が必須」と記録
- 評価レポート (本ファイル) への参照を Cargo.toml コメント末尾に追記

これにより:
- 将来の contributor が「この feature は本当に必要か?」と疑問に思った際、本評価レポートを参照して結論を得られる
- `actor-lock-construction-governance` capability の新規 Scenario「low-level utility crate の feature 指定は対象ターゲット / ユースケースが明示されている」を満たす状態になる

## step08 へのハンドオフ

step08 は **中止** (proposal の design 段階で「中止判断」を明記して archive、または proposal を archive に直接移動)。

中止理由メモを step08 archive 時に proposal 末尾へ追記:

> step07 評価により、`portable-atomic/critical-section` feature は CI 対象の `thumbv8m.main-none-eabi` で actor-core 内 AtomicU64 を構築するため必須と確認された (scripts/ci-check.sh:1056)。退役した場合 no-std CI ジョブが compile error で fail するため、本 change は中止する。詳細は `docs/plan/2026-04-22-portable-atomic-critical-section-evaluation.md` を参照。
