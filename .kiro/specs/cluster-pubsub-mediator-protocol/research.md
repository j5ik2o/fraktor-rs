# Research & Design Decisions

## Summary

- **Feature**: `cluster-pubsub-mediator-protocol`
- **Discovery Scope**: Extension / Complex Integration
- **Key Findings**:
  - 既存 `pub_sub` は local broker、delivery policy、batching producer、std delivery actor を持つが、DistributedPubSubMediator 相当の command protocol と registry gossip contract は未分離。
  - Pekko の mediator は path registry と topic registry を同じ bucket gossip で広めるが、fraktor-rs では core protocol と std delivery execution を分ける必要がある。
  - この仕様は upstream の membership active view と gossip envelope を利用する側であり、gossip substrate、downing、discovery、serialization framework を所有しない。

## Research Log

### 既存 fraktor-rs pubsub 実装

- **Context**: brownfield extension として、現在の core/adaptor 境界を崩さずに mediator protocol を追加する必要があった。
- **Sources Consulted**:
  - `modules/cluster-core-kernel/src/pub_sub.rs`
  - `modules/cluster-core-kernel/src/pub_sub/pub_sub_api.rs`
  - `modules/cluster-core-kernel/src/pub_sub/pub_sub_broker.rs`
  - `modules/cluster-core-kernel/src/pub_sub/cluster_pub_sub.rs`
  - `modules/cluster-adaptor-std/src/pub_sub/pub_sub_delivery_actor.rs`
- **Findings**:
  - `PubSubBroker` は topic と subscriber state を保持し、`ClusterPubSub` は subscribe / publish / topology update を扱う。
  - `PubSubDeliveryActor` は std 側で identity resolution と message delivery を実行する。
  - path-based `Send` / `SendToAll` と topic registry gossip はまだ独立した contract になっていない。
- **Implications**:
  - mediator command、settings、path registry、topic registry delta は core/pub_sub に追加する。
  - std adaptor は delivery intent の実行だけを持ち、protocol decision を持たない。

### Pekko DistributedPubSubMediator comparison

- **Context**: gap analysis の active follow-up は Pekko の mediator protocol と registry gossip を根拠にしている。
- **Sources Consulted**:
  - `references/pekko/cluster-tools/src/main/scala/org/apache/pekko/cluster/pubsub/DistributedPubSubMediator.scala`
  - `docs/gap-analysis/cluster-gap-analysis.md`
- **Findings**:
  - settings は role、routing logic、gossip interval、removed TTL、max delta elements、dead letters behavior を持つ。
  - mediator protocol は `Put` / `Remove`、`Subscribe` / `Unsubscribe`、`Publish`、`Send`、`SendToAll`、query command を含む。
  - registry は owner bucket、bucket version、entry version、status/delta exchange、removed entry pruning を使う。
- **Implications**:
  - Rust 側では public API parity ではなく、同じ operator-observable semantics を core contract として定義する。
  - routing logic は current `DeliveryPolicy` と衝突させず、path `Send` 用の bounded routing mode として扱う。

### Upstream dependency alignment

- **Context**: roadmap では pubsub mediator が `cluster-membership-reachability-model` と `cluster-gossip-heartbeat-protocol` に依存する。
- **Sources Consulted**:
  - `.kiro/specs/cluster-membership-reachability-model/requirements.md`
  - `.kiro/specs/cluster-membership-reachability-model/design.md`
  - `.kiro/specs/cluster-gossip-heartbeat-protocol/requirements.md`
  - `.kiro/specs/cluster-gossip-heartbeat-protocol/design.md`
- **Findings**:
  - membership spec は `UniqueAddress`、data center、active/removed member semantics、reachability matrix を提供する。
  - gossip spec は `GossipEnvelope`、payload kind、deadline、full gossip/heartbeat substrate を提供する。
  - gossip spec 自体が pubsub mediator と topic registry gossip を out of boundary として残している。
- **Implications**:
  - pubsub registry payload は gossip envelope に載せる payload として設計し、envelope framing は upstream に委譲する。
  - member removal と role filter は membership current state を入力にし、pubsub が membership merge semantics を所有しない。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Core mediator protocol + std delivery bridge | command/settings/registry/delta を core に置き、delivery execution を std adaptor に残す | no_std 境界と既存 pub_sub 構造に合う | core contract の追加ファイルが増える | 採用 |
| std mediator actor に protocol を寄せる | Pekko に近く、actor lifecycle と一体化しやすい | 実装は短く見える | core/adaptor 境界が崩れ、no_std contract が残らない | 不採用 |
| typed pubsub topic API だけを拡張する | typed API の利用者には見えやすい | 既存 topic surface と近い | cluster registry gossip と path semantics を表現しにくい | 不採用 |

## Design Decisions

### Decision: mediator semantics は core contract として定義する

- **Context**: protocol semantics を std actor に閉じると、no_std core と testable contract が失われる。
- **Alternatives Considered**:
  1. std actor に mediator behavior を実装する。
  2. core に command / state / outcome を定義し、std は delivery intent を実行する。
- **Selected Approach**: core/pub_sub に mediator command、settings、path semantics、registry bucket、delta collector を置く。
- **Rationale**: 既存の port-and-adapter 方針と一致し、gossip payload や settings を host runtime から独立して検証できる。
- **Trade-offs**: 初期実装では command 型と adapter bridge の接続が必要になる。
- **Follow-up**: `PubSubApi` と既存 `ClusterPubSub` trait との公開 surface を実装時に最小変更で接続する。

### Decision: pubsub registry gossip は gossip substrate の payload として扱う

- **Context**: upstream gossip spec が envelope / heartbeat / transport を所有する。
- **Alternatives Considered**:
  1. pubsub 専用 gossip transport を追加する。
  2. existing gossip envelope payload kind として registry status / delta を追加する。
- **Selected Approach**: pubsub は registry status / delta payload と version contract だけを所有し、dispatch は gossip substrate に委譲する。
- **Rationale**: transport duplicate を避け、downstream serialization contract とも衝突しない。
- **Trade-offs**: gossip envelope payload kind の追加時に upstream spec の revalidation が必要。
- **Follow-up**: `cluster-message-serialization-contract` は pubsub payload の wire representation を再確認する。

### Decision: Send / SendToAll は topic publish とは別の path registry delivery として扱う

- **Context**: Pekko mediator は topic publish と actor path send を同じ mediator に持つが、delivery target の意味は異なる。
- **Alternatives Considered**:
  1. path を topic name として扱い既存 publish に統合する。
  2. path registry entry と topic subscription entry を別 variant にする。
- **Selected Approach**: registry entry を path entry と topic subscription entry に分け、delivery mode で `Send` / `SendToAll` / `Publish` を区別する。
- **Rationale**: `Send` local affinity と `SendToAll` all-but-self を正しく表現できる。
- **Trade-offs**: registry delta の entry variant が増える。
- **Follow-up**: actor path canonicalization は actor-core の path semantics と整合させる。

## Risks & Mitigations

- Gossip payload kind が upstream spec と食い違う — design の revalidation trigger に明記し、payload framing は実装しない。
- Topic group semantics が既存 `PubSubBroker` とずれる — requirements では optional group を protocol entry として固定し、delivery policy の詳細は design に閉じる。
- registry delta が serializer spec を先取りする — data model は core state contract に留め、wire format は downstream serialization spec に残す。

## References

- `docs/gap-analysis/cluster-gap-analysis.md` — active follow-up の pubsub 項目。
- `references/pekko/cluster-tools/src/main/scala/org/apache/pekko/cluster/pubsub/DistributedPubSubMediator.scala` — mediator protocol と settings の comparison source。
- `.kiro/steering/tech.md` — no_std core と std adaptor separation。
- `.kiro/specs/cluster-membership-reachability-model/design.md` — membership identity と active member view。
- `.kiro/specs/cluster-gossip-heartbeat-protocol/design.md` — gossip envelope と payload substrate。
