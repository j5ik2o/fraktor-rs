## Why

`modules/actor-core/Cargo.toml:25` の `portable-atomic = { workspace = true, default-features = false, features = ["critical-section"] }` は、組み込み 32-bit ターゲット（`armv7m` など `AtomicU64` 非対応プラットフォーム）向けの `AtomicU64` fallback を有効化する目的で `critical-section` feature を指定している。

しかし以下が未確認のまま:
- fraktor-rs が実際に組み込み 32-bit ターゲットでビルド・運用される実績はあるか
- ある場合、どのターゲット（thumbv6m、thumbv7m、riscv32i 等）が対象か
- ない場合、`core::sync::atomic::AtomicU64` で十分ではないか

本 change は判断材料の収集・評価のみを行い、step08（実際の退役）の是非を確定する **調査 change**。実装フェーズで得られた結論に基づいて step08 に進むか、逆に step08 を中止して方針転換する。

Strategy B の第 7 ステップ（評価）。step08 と合わせて `portable-atomic/critical-section` 依存の処遇を決める。

## What Changes

本 change は実装を伴わない評価 change である。以下を生成物として残す:

- **Investigation report**（`docs/plan/` 配下に配置、`.takt/` ではない）:
  - fraktor-rs の CI 対象ターゲット棚卸し（`.github/workflows/**.yml` を調査）
  - `modules/actor-core/src/` および workspace 全体で `portable_atomic::AtomicU64` が使われている箇所を列挙
  - 各利用箇所が要求する整数幅の特定（`AtomicU64` なのか `AtomicUsize` なのか、`AtomicU32` で十分か）
  - `core::sync::atomic` で代替可能かの一次評価
  - 組み込み系 contributor の実利用報告があるか（GitHub issues / discussions を軽く調査）
- **Decision matrix**（`docs/plan/` 配下）:
  - 選択肢 X（`portable-atomic/critical-section` 退役）、Y（維持）、Z（条件付き維持: feature flag で切替）のトレードオフ比較
- **Recommendation**: step08 で採るべきアクションを明示

**Non-Goals**:
- コード変更は一切行わない（`Cargo.toml` の修正、`core::sync::atomic` への置換は step08 のスコープ）
- `portable-atomic` 本体依存の削除（`portable-atomic` は `heapless` 等からも使われているため、feature 指定の再評価のみを対象とする）
- 組み込み系の実ビルド検証（CI 外でのクロスコンパイル実行は step08 のスコープ）

## Capabilities

### New Capabilities
- なし（調査 change であり、capability の新設は不要）

### Modified Capabilities
- なし

OpenSpec validation 要件を満たすため、design / specs フェーズで最低 1 件の delta を設計する。候補:
- 案 A: 既存 `compile-time-lock-backend` に Scenario を追加（atomic backend の決定手順として）
- 案 B: 新規 capability `actor-core-dependency-governance` を ADDED し、「低レベル依存の feature 指定は用途ターゲットが明示されなければならない」ルールを明文化
- 案 C: 本 change は純粋な調査 change として spec delta を最小限にとどめ、step08 側で必要な spec 変更をまとめて行う

## Impact

- **Affected code**: なし（ドキュメント生成のみ）
- **Affected APIs**: なし
- **Affected dependencies**: なし
- **Release impact**: なし
- **Output artifacts**:
  - `docs/plan/YYYY-MM-DD-portable-atomic-critical-section-evaluation.md`（investigation report + decision matrix）
