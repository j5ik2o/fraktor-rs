# Research & Design Decisions

## Summary

- **Feature**: `cluster-gossip-heartbeat-protocol`
- **Discovery Scope**: Extension / Complex Integration
- **Key Findings**:
  - 既存 core は `GossipDisseminationCoordinator`、`GossipOutbound`、`VectorClock` による delta diffusion と ack tracking を持つが、from/to `UniqueAddress` を持つ envelope はない。
  - 既存 std adaptor は `TokioGossipTransport` と `GossipWireDeltaV1` により delta-only UDP wire を持つため、full gossip state、seen digest、heartbeat payload を区別する wire envelope が必要である。
  - upstream `cluster-membership-reachability-model` が `UniqueAddress`、data center、reachability matrix を定義するため、この spec はそれらを前提に liveness evidence と convergence contract を追加する。

## Research Log

### Existing gossip core

- **Context**: `GossipEnvelope` と full gossip merge の追加範囲を決めるため、既存 core membership の gossip 実装を確認した。
- **Sources Consulted**:
  - `modules/cluster-core-kernel/src/membership/gossip_dissemination_coordinator.rs`
  - `modules/cluster-core-kernel/src/membership/gossip_outbound.rs`
  - `modules/cluster-core-kernel/src/membership/vector_clock.rs`
  - `modules/cluster-core-kernel/src/membership/membership_delta.rs`
- **Findings**:
  - `GossipOutbound` は target authority と `MembershipDelta` のみを持つ。
  - coordinator は peer versions、seen set、vector clock を保持し、delta diffusion / reconcile / ack を扱う。
  - `MembershipDelta` は version range と updated `NodeRecord` list を持つが、full state と tombstone はない。
- **Implications**:
  - `GossipEnvelope` は既存 `GossipOutbound` を拡張または置換する protocol boundary になる。
  - seen digest は既存 seen/vector clock の概念を reusable にするが、full state merge と tombstone retention も同じ convergence contract に含める必要がある。

### Existing std wire transport

- **Context**: std adaptor がどこまで責務を持つべきかを確認した。
- **Sources Consulted**:
  - `modules/cluster-adaptor-std/src/membership/tokio_gossip_transport.rs`
  - `modules/cluster-adaptor-std/src/membership/gossip_wire_delta_v1.rs`
  - `modules/cluster-adaptor-std/src/membership.rs`
- **Findings**:
  - std transport は UDP socket、Tokio task、allowed peer filtering、transport send/receive を持つ。
  - wire shape は `GossipWireDeltaV1` のみで、payload kind や heartbeat request / response は表現できない。
  - decode failure は `GossipTransportError` に変換できる既存 pattern がある。
- **Implications**:
  - std 側は logical envelope handoff までを扱い、versioned bytes は cluster-message-serialization-contract に残す。
  - merge rule、tombstone rule、seen digest rule は std に置かず、core に戻す。

### Upstream membership and reachability dependency

- **Context**: この spec が membership model と重複しない境界を確認した。
- **Sources Consulted**:
  - `.kiro/specs/cluster-membership-reachability-model/requirements.md`
  - `.kiro/specs/cluster-membership-reachability-model/design.md`
  - `.kiro/steering/roadmap.md`
- **Findings**:
  - upstream spec は `UniqueAddress`、data center、`WeaklyUp`、Reachability matrix、indirect connection evidence を所有する。
  - upstream spec は gossip envelope、heartbeat protocol、full gossip merge、tombstone、seen digest を downstream に残している。
- **Implications**:
  - この spec は upstream の data center と reachability matrix を consume し、identity model を再定義しない。
  - downing policy は evidence consumer であり、この spec では実行しない。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Delta-only extension | 既存 `GossipOutbound` に heartbeat field を追加する | 変更量が少ない | full merge、tombstone、seen digest、payload kind が曖昧になる | 不採用 |
| Core protocol + std wire adaptor | core に envelope / merge / heartbeat semantics、std に wire encode/decode を置く | no_std 境界と責務分離を維持できる | `GossipTransport` API の移行が必要 | 採用 |
| Generic serializer-first | cluster message serializer contract を先に作り gossip payload を載せる | 将来の serializer 統一に近い | 現 spec が serialization contract を吸収する | 不採用 |

## Design Decisions

### Decision: GossipEnvelope を core contract にする

- **Context**: target-only `GossipOutbound` では from/to identity、payload kind、deadline を検証できない。
- **Alternatives Considered**:
  1. `GossipOutbound` に field を追加する。
  2. `GossipEnvelope` を新しい protocol value として導入する。
- **Selected Approach**: `GossipEnvelope` を core/membership に置き、std transport は logical envelope handoff を扱う。
- **Rationale**: identity と payload kind は membership semantics であり、std transport の endpoint と分離して検証する必要がある。
- **Trade-offs**: 既存 `GossipTransport` の migration が必要になるが、downstream pubsub / serialization spec との境界が明確になる。
- **Follow-up**: 実装時に backward-compatible adapter が必要かを targeted tests で判断する。

### Decision: full gossip merge と tombstone は membership core が所有する

- **Context**: tombstone と seen digest は membership convergence の一部であり、wire codec や downing policy ではない。
- **Alternatives Considered**:
  1. `MembershipTable::apply_delta` に全て集約する。
  2. `GossipStateModel` 相当の contract に full state / tombstone / seen digest をまとめる。
- **Selected Approach**: `GossipStateModel` を core/membership の責務として設計し、`MembershipTable` は state access と apply の対象にする。
- **Rationale**: delta apply と full merge/tombstone/seen digest は近いが、単純な table mutation だけでは convergence events を表現しにくい。
- **Trade-offs**: model が増えるが、task boundary と review scope が明確になる。
- **Follow-up**: 実装時に既存 coordinator へ統合するか、独立 helper として保持するかを小さく判断する。

### Decision: heartbeat は evidence producer に限定する

- **Context**: heartbeat miss は reachability と downing に影響するが、downing decision をここに含めると scope が肥大化する。
- **Alternatives Considered**:
  1. heartbeat timeout から直接 member status を `Suspect` / `Dead` にする。
  2. heartbeat timeout を reachability matrix input として公開する。
- **Selected Approach**: dedicated heartbeat は request / response / timeout evidence を生成し、status transition や downing policy は downstream boundary に残す。
- **Rationale**: upstream reachability model と downing SBR spec の責務を保てる。
- **Trade-offs**: integration task では evidence consumer の接続確認が必要。
- **Follow-up**: `MembershipCoordinator` の既存 `handle_heartbeat` との migration path を実装時に明確化する。

## Risks & Mitigations

- `GossipTransport` API migration が広がる — envelope-aware adapter を最小単位で導入し、既存 delta tests を roundtrip tests に置き換える。
- full state payload が大きくなる — std transport の payload size limit と oversized decode rejection を必須にする。
- Cross-DC heartbeat が routing/discovery/downing へ広がる — requirements/design/tasks に scope guard を置き、evidence 生成だけに限定する。
- tombstone retention が premature prune になる — seen digest convergence と active peer set を prune prerequisite にする。

## References

- `docs/gap-analysis/cluster-gap-analysis.md` — `GossipEnvelope`、dedicated heartbeat、full `Gossip` merge / tombstone / seen digest、`CrossDcClusterHeartbeat` の comparison evidence。
- `.kiro/specs/cluster-membership-reachability-model` — upstream identity / data center / reachability contract。
- `modules/cluster-core-kernel/src/membership/gossip_dissemination_coordinator.rs` — existing gossip coordinator。
- `modules/cluster-adaptor-std/src/membership/tokio_gossip_transport.rs` — existing std transport。
