# Research & Design Decisions

## Summary

- **Feature**: `cluster-active-compatibility-baseline`
- **Discovery Scope**: Extension
- **Key Findings**:
  - `ClusterApi::remote_path_of` と remoting lifecycle subscription retention は既存実装とテストがあり、baseline spec では contract を固定して gap analysis 追跡対象にする。
  - `ClusterExtensionConfig` は pubsub、downing provider key、SBR settings の join compatibility をすでに比較しているが、required key filtering、sensitive key exclusion、checker composition、failure detector choice は追加 surface として整理が必要である。
  - `ClusterExtensionInstaller::with_downing_provider_factory` は compatibility metadata として存在するため、std 側の `DowningProviderCompatibility` は full decision model ではなく metadata surface に限定する。

## Research Log

### gap analysis scope

- **Context**: `Active comparison follow-up: trivial / easy` の4項目を baseline にするため、完了対象と downstream 対象を確認した。
- **Sources Consulted**: `docs/gap-analysis/cluster-gap-analysis.md`
- **Findings**: 対象は `DowningProviderCompatibility`、config compatibility full key set、`remotePathOf`、transport lifecycle bridge retention。`WeaklyUp`、`Reachability`、gossip、downing strategy、discovery、pubsub、serialization は別 spec に分離済み。
- **Implications**: design と tasks は4項目以外を実装対象に含めない。gap analysis 更新もこの4項目に限定する。

### existing cluster core and std surfaces

- **Context**: baseline が既存境界に沿うかを確認した。
- **Sources Consulted**: `modules/cluster-core-kernel/src/extension/cluster_extension_config.rs`, `modules/cluster-core-kernel/src/topology/join_config_compat_checker.rs`, `modules/cluster-core-kernel/src/downing_provider/*`, `modules/cluster-core-kernel/src/extension/cluster_api.rs`, `modules/cluster-adaptor-std/src/cluster_provider/local_cluster_provider_ext.rs`
- **Findings**: core は `ClusterExtensionConfig`、`JoinConfigCompatChecker`、`DowningProviderCompatibility`、`SplitBrainResolverSettings` を持つ。std adaptor は `subscribe_remoting_events` で `EventStreamSubscription` を返し、weak provider handle を使って provider を強保持しない。
- **Implications**: 新規 design は core/config、extension/api、downing_provider、std/provider の既存境界を変更せずに拡張する。

### upstream specs

- **Context**: 既存 OpenSpec との重複を避けるため、touchpoint を確認した。
- **Sources Consulted**: `openspec/specs/cluster-provider-boundary/spec.md`, `openspec/specs/cluster-adaptor-std-remote-delivery/spec.md`, `openspec/specs/cluster-core-module-organization/spec.md`, `openspec/specs/cluster-grain-runtime-operational-contract/spec.md`
- **Findings**: provider boundary は lifecycle/discovery input を topology input に変換する責務を定義済み。remote delivery spec は remoting subscription retention を既に要求済み。module organization は topology/extension/downing_provider の境界を分けている。
- **Implications**: この spec は既存要求を置き換えず、comparison-driven baseline と gap close-out の追跡性を追加する。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Existing boundary extension | core/config、extension/api、std/provider の既存 surface を最小拡張する | 既存 tests と crate boundary を使える | 新規概念名が散らばる可能性 | 採用 |
| New compatibility module | `compatibility` module を新設して4項目を束ねる | spec 名と一致しやすい | 既存 `topology` / `extension` / `downing_provider` 境界と重複する | 不採用 |
| Full Pekko parity layer | Pekko API 名をまとめて再現する | 比較表とは対応しやすい | roadmap scope を超え、Deferred concepts を吸収しやすい | 不採用 |

## Design Decisions

### Decision: baseline は既存境界への小さな追加として扱う

- **Context**: 4項目は単一 runtime component ではなく config、path、provider、lifecycle にまたがる。
- **Alternatives Considered**:
  1. baseline 専用 module を追加する。
  2. 既存境界に契約を追加し、spec で束ねる。
- **Selected Approach**: 既存境界に追加し、design/tasks の `_Boundary:_` で責務を分ける。
- **Rationale**: `cluster-core-module-organization` が定義した境界を壊さず、downstream spec が個別に参照できる。
- **Trade-offs**: spec は4つの小 surface を扱うため、tasks で boundary を明示する必要がある。
- **Follow-up**: 実装時に gap analysis の対象行だけを更新する。

### Decision: downing provider compatibility metadata は provider compatibility baseline に留める

- **Context**: `DowningProviderCompatibility` は easy 項目だが、SBR decision model は hard 項目で downstream にある。
- **Alternatives Considered**:
  1. SBR actor / strategy 判定まで実装する。
  2. provider key、settings identity、factory/helper だけを固定する。
- **Selected Approach**: provider-facing factory/helper と compatibility metadata を固定する。
- **Rationale**: downing decision model、reachability、lease majority を吸収しない。
- **Trade-offs**: 実際の SBR behavior は no-op または caller-supplied provider に依存する。
- **Follow-up**: `cluster-downing-sbr-decision-model` が decision behavior を所有する。

## Risks & Mitigations

- config compatibility が broad になりすぎる — key catalog、sensitive exclusion、checker composition に限定し、dynamic loader や HOCON parity は対象外にする。
- `remote_path_of` が actor-core path ownershipを壊す — `ActorPath` / `ActorPathParser` を使い、cluster API は helper surface に留める。
- lifecycle retention が provider をリークさせる — weak provider handle と explicit subscription guard drop を acceptance と tests に含める。

## References

- `docs/gap-analysis/cluster-gap-analysis.md`
- `openspec/specs/cluster-provider-boundary/spec.md`
- `openspec/specs/cluster-adaptor-std-remote-delivery/spec.md`
- `modules/cluster-core-kernel/src/extension/cluster_api.rs`
- `modules/cluster-adaptor-std/src/cluster_provider/local_cluster_provider_ext.rs`
