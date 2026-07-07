## 1. Kernel settings contract

- [x] 1.1 Add `ClusterShardingSettings` with number_of_shards, role, passivation, remember_entities, tuning
- [x] 1.2 Add `PassivationStrategy` enum and validation
- [x] 1.3 Wire into `ClusterExtensionConfig::validate()`

## 2. Runtime integration

- [x] 2.1 Add `VirtualActorRegistry::passivate_by_strategy`
- [x] 2.2 Add `ShardingQueryHandler` for local observability queries

## 3. Typed facade

- [x] 3.1 Add typed `ClusterSharding` / `Entity<M>` / `EntityRegion<M>`

## 4. Verification

- [x] 4.1 Unit tests for settings, passivation, query handler
- [x] 4.2 Showcase example in `showcases/std`
