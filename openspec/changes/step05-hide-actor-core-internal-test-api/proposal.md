## Why

`actor-core/test-support` feature の責務 C は「内部 API の `pub(crate)` → `pub` 格上げ」である。具体的には `Behavior::handle_message`、`ActorCell::receive` のような、本来 crate 内部に閉じたい API が、ダウンストリームのテスト（`fraktor-cluster-*`、`fraktor-remote-*` の統合テスト等）のために `#[cfg(any(test, feature = "test-support"))]` ゲート内で `pub` になっている。

step04 で `fraktor-actor-test-rs` crate を切り出した後、この責務 C を解消する戦略は 3 つ:

- **A. 内部 API を本当に公開する** → Rust の `pub` は crate 境界で切れるため、ダウンストリームから必要なら正規の public API として設計し直す
- **B. `fraktor-actor-test-rs` 経由で安全に露出する** → `actor-test` crate 内で薄いファサード関数を提供し、internal API を呼び出す（`actor-core` 側は `pub(crate)` に戻せる）
- **C. テスト設計自体を見直す** → 本当に internal API を叩く必要があるのか再評価。多くの場合は public API（`ActorRef::tell` 等）でカバーできる

本 change は Strategy B の第 5 ステップ（責務 C 処理）。ケースごとに A/B/C のいずれを採るかを design で判断し、ほぼすべての `#[cfg(any(test, feature = "test-support"))]` による `pub` 格上げを排除する。

## What Changes

- `actor-core` 配下で `#[cfg(any(test, feature = "test-support"))]` により可視性を拡大している全箇所を棚卸し（Grep で全数収集）
- 各箇所について A/B/C のいずれかの戦略を割り当て（design 段階で決定）
- 割り当てに従いリファクタ:
  - A: 正規 public API として docs / 型シグネチャを整備
  - B: `actor-test` crate 側にファサード関数を追加し、`actor-core` 側は `pub(crate)` に戻す
  - C: ダウンストリームのテストを public API 経由に書き換え、`actor-core` 側の unrealistic な露出を削除
- `actor-core/src/` から `#[cfg(any(test, feature = "test-support"))]` で可視性拡大している箇所が 0 件になるのを目標にする（純粋な `#[cfg(test)]` は保持）
- **BREAKING（workspace-internal）**: 一部 API のパス・シグネチャ・可視性が変わる（ダウンストリームテストの修正が必要）

**Non-Goals**:
- `test-support` feature 自体の削除は step06 で行う（本 change 完了後は feature が空 `[]` または限りなく空に近づく想定）
- `actor-core` の public API surface の再設計（責務 C 解消で露出する必要のある API のみ整備）

## Capabilities

### New Capabilities
- なし

### Modified Capabilities
- なし

OpenSpec validation 要件を満たすため、design / specs フェーズで最低 1 件の delta を設計する。候補:
- 案 A: 新規 capability `actor-core-api-visibility-governance` を ADDED し、「feature flag 経由で内部 API の可視性を拡大してはならない」ルールを明文化
- 案 B: `actor-lock-construction-governance` に同趣旨の Scenario を追加

## Impact

- **Affected code**:
  - `modules/actor-core/src/**` の各所（`#[cfg(any(test, feature = "test-support"))]` 削除と可視性戻し）
  - `modules/actor-test/src/**`（ファサード関数追加、戦略 B 採用箇所）
  - ダウンストリームのテスト（`modules/cluster-*/tests/`、`modules/remote-*/tests/` 等）の書き換え
- **Affected APIs**: workspace-internal な API シグネチャ・可視性変更
- **Affected dependencies**: なし（crate 依存構造は step04 で確定した形を維持）
- **Release impact**: pre-release phase につき外部影響は軽微
