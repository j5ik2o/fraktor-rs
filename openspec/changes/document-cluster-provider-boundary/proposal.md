## Why

Grain runtime の identity / topology / passivation contract は固定できたが、その入力を作る provider 側の責務境界がまだ散らばっている。local / static / AWS ECS provider が何を membership input として供給し、どこから cluster core が扱うのかを明文化してから downing / reachability / rebalance に進む。

## What Changes

- local / static / AWS ECS provider の membership input boundary を仕様化する。
- `cluster-core` が provider-specific discovery を知らず、`TopologyUpdated` / member departure input だけを扱うことを仕様化する。
- `cluster-core` が定義する provider port と、std adapter が実装する remoting lifecycle subscription / AWS ECS polling lifetime の保持責務を文書化する。
- provider boundary を docs に整理し、既存 provider tests で守られている契約を明示する。
- `DowningProvider` の decision model、SBR、reachability matrix、rebalance、remembered entities は含めない。

## Capabilities

### New Capabilities

- `cluster-provider-boundary`: Cluster provider が membership / lifecycle / discovery input を cluster core へ渡す責務境界を定義する。

### Modified Capabilities

- なし

## Impact

- `openspec/specs/cluster-provider-boundary/spec.md`
- `modules/cluster-core/src/cluster_provider/`
- `modules/cluster-adaptor-std/src/local_cluster_provider_ext.rs`
- `modules/cluster-adaptor-std/src/aws_ecs_cluster_provider.rs`
- `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md`
- provider boundary に関係する既存テスト
