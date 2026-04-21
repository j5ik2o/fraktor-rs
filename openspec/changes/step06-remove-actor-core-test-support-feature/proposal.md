## Why

step01〜step05 を経て `actor-core/test-support` feature が抱えていた 3 責務（A: critical-section/std impl provider、B: ダウンストリーム統合テスト用 API、C: 内部 API 可視性拡大）はすべて退役済みの状態になる想定。`actor-core/Cargo.toml:19` の `test-support = []` は空の名前空間として残ったまま、何の意味も持たない。

この空 feature は歴史的理由で残されているに過ぎず、残しておくと:
- 「何か意味があるのでは」と誤解した新規 contributor が再利用しようとする
- `actor-core` の `[[test]]` で `required-features = ["test-support"]` が書かれ続ける（現在 8 箇所）
- ダウンストリームの `Cargo.toml` で `features = ["test-support"]` が残り続け、いつか壊れる

最終ステップとして feature 自体を削除し、`actor-core` の feature surface を整理する。本 change は Strategy B の第 6 ステップであり、test-support 関連タスクの打ち止め。

## What Changes

- `modules/actor-core/Cargo.toml`:
  - `[features]` セクションから `test-support = []` 行を削除
  - `[[test]]` セクション群（8 箇所）の `required-features = ["test-support"]` 行を削除
- ダウンストリームの `Cargo.toml`:
  - `fraktor-actor-core-rs = { ..., features = ["test-support"] }` の `features = ["test-support"]` 指定を全削除
  - 対象: `modules/cluster-*`、`modules/remote-*`、`modules/stream-*`、`modules/persistence-*`、`showcases/std` の `[dependencies]` および `[dev-dependencies]`
- `actor-core/src/` 内で `feature = "test-support"` を参照する `#[cfg(...)]` が残っていれば削除（step05 で 0 件になっている想定だが検証する）
- `docs/plan/2026-04-21-actor-core-critical-section-followups.md` の残課題 1 を「解消済み」に更新
- **BREAKING（workspace-internal、ほぼ影響なし）**: 存在しない feature の指定が `Cargo.toml` に残っていても pre-release phase では検出されやすい

**Non-Goals**:
- `actor-adaptor-std` 等の他クレートの `test-support` feature 見直し（独自責務を持つため個別判断）
- `actor-core` の `alloc` / `alloc-metrics` feature の見直し（別スコープ）

## Capabilities

### New Capabilities
- なし

### Modified Capabilities
- なし

OpenSpec validation 要件を満たすため、design / specs フェーズで最低 1 件の delta を設計する。候補:
- 案 A: step04 / step05 で導入した capability（`actor-test-helpers-placement` / `actor-core-api-visibility-governance` 等）に Scenario を追加し、「`actor-core` には `test-support` feature が存在しない」を検査項目化
- 案 B: 既存 `actor-lock-construction-governance` に Scenario を追加

## Impact

- **Affected code**:
  - `modules/actor-core/Cargo.toml`（feature 削除、`[[test]]` required-features 削除）
  - 全ダウンストリーム crate の `Cargo.toml`
- **Affected APIs**: なし（既に step03-05 で移設・再設計済み）
- **Affected dependencies**: なし
- **Release impact**: pre-release phase につき外部影響は軽微。`fraktor-actor-core-rs` の feature surface が縮小する
