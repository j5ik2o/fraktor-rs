## Why

`2026-04-20-pekko-panic-guard` の完了確認で、change の追加スコープとは無関係な既存項目が `src/` 配下 test module と repo-wide `dead_code` 条件に引っかかることが明確になった。panic-guard 自体は完了しているため、production code と test-only code の境界整理は独立した健全性改善として切り出す。

## What Changes

- `src/` 配下に残っている std 依存の test module を洗い出し、必要に応じて `tests/` へ移動する
- production module から見て不要な test-only helper / type / method を整理し、repo-wide `dead_code` 条件に近づける
- no_std 制約を持つ core crate で、production path と test path の境界を明確化する
- **BREAKING なし**。目的は公開 API の変更ではなく、repo 内部の test 配置と健全性ルールの整理

## Capabilities

### New Capabilities
- `source-test-layout-hygiene`: production source tree と test-only code の境界を整理し、`src/` 配下の test module 配置ルールを明確化する

### Modified Capabilities

## Impact

- 対象コード:
  - `modules/*/src/**` にある `#[cfg(test)] mod tests;` と対応する `tests.rs`
  - `modules/*/tests/**` への移設候補
  - repo-wide `clippy -D dead_code` に影響する test-only helper
- 影響範囲:
  - 主に actor-core を起点とした既存 test module の配置整理
  - 公開 API / runtime semantics への影響は持ち込まない
  - `src/` 配下の no_std / std 境界に関する判定条件の明文化
