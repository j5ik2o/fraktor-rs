# Research & Design Decisions

## Summary

- **Feature**: `cluster-message-serialization-contract`
- **Discovery Scope**: Extension
- **Key Findings**:
  - `actor-core` には `SerializedMessage`、`SerializerId`、manifest-aware serializer、`SerializationExtension` が既にあるため、cluster 側で serializer registry を複製しない。
  - upstream gossip spec は `GossipEnvelope` payload kind と logical transport handoff を持つが、generic serializer framework は downstream scope として残している。
  - upstream pubsub spec は `TopicRegistryGossipPayload` を定義するが、serializer framework と wire compatibility は downstream scope として残している。

## Research Log

### actor-core serialization boundary
- **Context**: cluster message serializer contract が actor-core serialization とどう接続するかを確認した。
- **Sources Consulted**:
  - `modules/actor-core-kernel/src/serialization.rs`
  - `modules/actor-core-kernel/src/serialization/serializer.rs`
  - `modules/actor-core-kernel/src/serialization/string_manifest_serializer.rs`
  - `modules/actor-core-kernel/src/serialization/serialized_message.rs`
  - `modules/actor-core-kernel/src/serialization/extension.rs`
- **Findings**:
  - `SerializedMessage` は serializer id、optional manifest、bytes を持つ。
  - `SerializerWithStringManifest` は manifest 付き deserialize を提供する。
  - `SerializationExtension` は `serialize` / `deserialize` と registry lookup failure を既に扱う。
- **Implications**:
  - cluster core は actor-core serialized metadata をそのまま包む bridge model に留める。
  - serializer registry、serializer trait、setup builder はこの spec で再設計しない。

### upstream gossip / pubsub alignment
- **Context**: message serialization spec が upstream semantics を吸収しないよう、依存先 design を確認した。
- **Sources Consulted**:
  - `.kiro/specs/cluster-gossip-heartbeat-protocol/design.md`
  - `.kiro/specs/cluster-pubsub-mediator-protocol/design.md`
- **Findings**:
  - gossip spec は `GossipEnvelope`、payload kind、logical transport handoff を持つが serializer framework は out of boundary。
  - pubsub spec は `TopicRegistryGossipPayload` を持つが serializer framework と wire schema は out of boundary。
- **Implications**:
  - この spec は payload kind と serialized metadata だけを扱い、gossip merge や pubsub mediator state を実行しない。
  - downstream revalidation trigger に gossip/pubsub payload kind 変更を含める。

### std/wire bridge
- **Context**: cluster std adaptor で既存 wire codec pattern を確認した。
- **Sources Consulted**:
  - `modules/cluster-adaptor-std/src/membership/gossip_wire_delta_v1.rs`
  - `modules/cluster-adaptor-std/src/membership/tokio_gossip_transport.rs`
  - `modules/remote-core/src/wire/envelope_payload.rs`
- **Findings**:
  - cluster membership std wire は serde/postcard による dedicated wire shape を使っている。
  - remote envelope payload は serializer id、manifest、bytes を保持する。
- **Implications**:
  - std adaptor に `ClusterWireFrameV1` と codec を置き、transport lifecycle から独立させる。
  - frame は protobuf互換ではなく fraktor-rs runtime contract として扱う。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| actor-core reuse + cluster kind wrapper | `SerializedMessage` を source of truth にし、cluster payload kind を追加する | registry 重複がない、既存 diagnostics と一致する | kind/manifest validation が別途必要 | 採用 |
| cluster 専用 serializer registry | cluster-core に独自 registry を持つ | cluster messages だけを閉じられる | actor-core と二重管理になり drift する | 不採用 |
| protobuf-first wire schema | proto schema を source of truth にする | 外部互換を議論しやすい | 現 scope の protobuf 完全互換と衝突する | 不採用 |

## Design Decisions

### Decision: `SerializedMessage` を source of truth にする
- **Context**: actor-core に serializer id、manifest、payload bytes の既存 contract がある。
- **Alternatives Considered**:
  1. cluster 専用 serialized message を独自 field で定義する。
  2. actor-core `SerializedMessage` を cluster payload kind と組み合わせる。
- **Selected Approach**: `ClusterSerializedMessage` が payload kind と `SerializedMessage` を保持する。
- **Rationale**: actor-core serialization の registry と diagnostics を再利用でき、cluster 側の registry 重複を避けられる。
- **Trade-offs**: actor-core serialization public API が変わると cluster message bridge の revalidation が必要になる。
- **Follow-up**: 実装時に `SerializationCallScope` の選択を remote/cluster 用に確認する。

### Decision: std wire frame は version one として明示する
- **Context**: unknown version と unknown payload を silent success にしない必要がある。
- **Alternatives Considered**:
  1. postcard の enum decode error に任せる。
  2. frame header に version と payload kind を置いて先に検査する。
- **Selected Approach**: `ClusterWireFrameV1` と decode failure taxonomy を定義する。
- **Rationale**: unsupported version と unknown payload kind を区別でき、future compatibility の revalidation trigger が明確になる。
- **Trade-offs**: frame header の field allocation を保守する必要がある。
- **Follow-up**: 実装時に existing membership wire tag と衝突しない numeric allocation を固定する。

## Risks & Mitigations

- actor-core serialization の再設計へ scope が広がる — bridge API だけを定義し、registry / serializer trait は変更しない。
- gossip/pubsub semantics を decode 時に実行してしまう — design と tasks で boundary guard を明示し、wire bridge は payload bytes を解釈しない。
- protobuf compatibility を暗黙に約束してしまう — requirements/design で完全 binary compatibility を out of boundary に固定する。

## References

- `modules/actor-core-kernel/src/serialization/serialized_message.rs` — actor-core serialized metadata。
- `modules/actor-core-kernel/src/serialization/extension.rs` — serialization extension の serialize / deserialize。
- `.kiro/specs/cluster-gossip-heartbeat-protocol/design.md` — gossip payload upstream contract。
- `.kiro/specs/cluster-pubsub-mediator-protocol/design.md` — pubsub payload upstream contract。
- `docs/gap-analysis/cluster-gap-analysis.md` — active follow-up source。
