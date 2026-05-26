## Why

Provider boundary が topology / departure input の入口を固定したので、次は既存の failure detector / membership / downing 実装を failure observation から explicit down / departure input へ進む最小 contract として整理する。SBR や reachability matrix へ広げる前に、suspect / unreachable / downing decision の責務を分けておく。

## What Changes

- 既存の failure detector / membership coordinator が生成する suspect / reachable observation を仕様化する。
- 既存の `DowningProvider` explicit down hook を、failure observation に対する decision boundary へ最小拡張する。
- Grain runtime へ渡る member departure input と、downing decision policy の責務境界を明文化する。
- 既存実装で contract を満たせない箇所だけ、最小の型・テスト・文書を追加する。
- SBR、reachability matrix、rebalance、remembered entities は対象外にする。

## Capabilities

### New Capabilities

- `failure-downing-minimum`: failure observation、suspect / unreachable state、downing decision、member departure input の最小責務境界を定義する。

### Modified Capabilities

- なし

## Impact

- `modules/cluster-core/src/downing_provider/`
- `modules/cluster-core/src/failure_detector/`
- `modules/cluster-core/src/membership/`
- `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md`
- failure detector / membership / downing provider に関係する既存テスト
