## Why

step07 の調査結果（`docs/plan/YYYY-MM-DD-portable-atomic-critical-section-evaluation.md`）で「`portable-atomic/critical-section` feature は不要」と結論されたら、本 change で実際に退役する。

退役の意義:
- `actor-core` が間接的に `critical-section` impl provider を要求する経路が消える（残課題が完全に閉じる）
- 依存グラフが簡素化される（`portable-atomic` の internal config が `critical-section` 不要に倒れる）
- step01〜step06 で確立した「`actor-core` は同期プリミティブ供給責任を持たない」原則が atomic 層にも徹底される

step07 で「維持すべし」となった場合、本 change は **中止または縮小** する。実装フェーズに入る前に design 段階で結論を再確認する。

Strategy B の最終ステップ（第 8 ステップ）。これで `actor-core` 周辺の同期プリミティブ依存に関する整理計画が完了する。

## What Changes

step07 の結論が「退役」だった場合のみ実施する変更:

- `modules/actor-core/Cargo.toml:25` の `portable-atomic = { ..., features = ["critical-section"] }` から `"critical-section"` を削除
  - 完全に不要なら `default-features = false` のみに整理
  - 一部ターゲットで必要なら、`actor-core` の feature flag（例: `portable-atomic-critical-section`）に optional 化して条件付き有効化
- `modules/actor-core/src/` で `AtomicU64`（または該当する atomic 型）の利用箇所を `core::sync::atomic` 由来に置換可能か再検証し、置換
- 組み込み系の主要ターゲット（step07 で特定したもの）でクロスコンパイル検証（CI に追加できるかも検討）
- `docs/plan/2026-04-21-actor-core-critical-section-followups.md` の残課題 2 を「解消済み」または「方針転換により取り下げ」に更新
- workspace の `cargo tree -e features` 等で `critical-section` impl provider 要求経路が完全に消えることを確認
- **BREAKING（workspace-internal）**: 組み込み 32-bit ターゲット向けビルドの構成が変わる可能性（feature 名・`Cargo.toml` 指定）

**Non-Goals**:
- step07 で「維持」と結論されたら本 change は中止（`design.md` で「中止判断」を明記して archive、または `proposal.md` 段階で削除）
- `heapless` 側の `portable-atomic` feature 指定見直し（別スコープ）

## Capabilities

### New Capabilities
- なし

### Modified Capabilities
- なし

OpenSpec validation 要件を満たすため、design / specs フェーズで最低 1 件の delta を設計する。候補:
- 案 A: `actor-lock-construction-governance` Requirement「actor-\* の Cargo.toml は primitive lock crate を non-optional な直接依存として宣言してはならない」の例外条項（`portable-atomic` のような low-level utility crate が引き込む推移的依存）を縮小・削除する MODIFIED
- 案 B: 既存 `compile-time-lock-backend` に「atomic backend は core::sync::atomic を第一選択とする」原則を Scenario として追加

## Impact

- **Affected code**:
  - `modules/actor-core/Cargo.toml`（feature 指定整理）
  - `modules/actor-core/src/` で `portable_atomic::AtomicU64` 等を使っている 16 ファイル相当（step07 調査範囲）
- **Affected APIs**: なし（atomic 型は内部実装詳細）
- **Affected dependencies**: `portable-atomic/critical-section` feature が無効化される
- **Release impact**:
  - 組み込み 32-bit ターゲット利用者が現れた場合の互換性は要評価（pre-release phase につき影響軽微）
  - workspace 全体で `critical-section` impl provider 要求が actor-core 側からは完全に消える
