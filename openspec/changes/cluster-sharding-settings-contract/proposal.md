## Why

Pekko `ClusterShardingSettings` parity requires a comprehensive sharding configuration contract owned by the cluster extension, including passivation strategy, number of shards, and remember-entities settings. Config-only fields without runtime wiring would increase unconnected public surface.

## What Changes

- Add `ClusterShardingSettings`, `PassivationStrategy`, and validation errors to `cluster-core-kernel`.
- Wire `ClusterExtensionConfig::sharding_settings()` and `validate()` delegation.
- Integrate passivation strategy execution into `VirtualActorRegistry::passivate_by_strategy`.
- Add `ShardingQuery` / `ShardingQueryHandler` for shard region observability.
- Add typed `ClusterSharding` / `Entity<M>` facade in `cluster-core-typed`.

## Capabilities

### New Capabilities
- `cluster-sharding-settings-contract`: Comprehensive sharding settings and passivation strategy contract.

### Modified Capabilities
- `cluster-grain-runtime-operational-contract`: Passivation strategy selection extends operational contract.

## Impact

- `modules/cluster-core-kernel/src/extension/`
- `modules/cluster-core-kernel/src/grain/`
- `modules/cluster-core-kernel/src/activation/virtual_actor_registry.rs`
- `modules/cluster-core-typed/src/cluster_sharding.rs`
