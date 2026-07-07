# cluster-sharding-settings-contract Specification

## Purpose
Define comprehensive cluster sharding settings owned by the cluster extension, including passivation strategy and remember-entities configuration.

## Requirements

### Requirement: Cluster sharding settings are validated at install boundary

`ClusterShardingSettings` SHALL validate `number_of_shards > 0`, passivation strategy limits, and remember-entities compatibility before cluster extension install.

#### Scenario: zero number of shards is rejected

- **WHEN** `ClusterShardingSettings` is configured with `number_of_shards = 0`
- **THEN** validation returns `ClusterShardingSettingsError::ZeroNumberOfShards`

### Requirement: Passivation strategy is applied by the activation registry

`VirtualActorRegistry` SHALL apply the configured `PassivationStrategy` when polled, passivating idle or excess activations according to the selected strategy.

#### Scenario: idle strategy passivates stale activations

- **GIVEN** an activation whose last_seen exceeds the idle timeout
- **WHEN** `passivate_by_strategy(Idle { .. }, now)` is invoked
- **THEN** the activation is removed and a Passivated event is emitted
