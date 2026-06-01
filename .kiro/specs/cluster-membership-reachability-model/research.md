# Research & Design Decisions

## Summary

- **Feature**: `cluster-membership-reachability-model`
- **Discovery Scope**: Extension
- **Key Findings**:
  - `fraktor-remote-core-rs` には Pekko `Address` / `UniqueAddress` 相当の `Address` と `UniqueAddress` が既にあり、cluster membership は同じ node incarnation semantics を再利用できる。
  - `NodeRecord` は現在 `node_id` と `authority` を持つが、address + uid の identity、data center、`WeaklyUp` は持たない。
  - 現在の reachability は `NodeStatus::Suspect` と unreachable event 中心で、Pekko の observer / subject / status / version を持つ matrix ではない。

## Research Log

### gap analysis scope

- **Context**: active medium follow-up のうち、membership/reachability model に属する項目だけを抽出した。
- **Sources Consulted**: `docs/gap-analysis/cluster-gap-analysis.md`
- **Findings**: 対象は `UniqueAddress` semantics、data center membership、`WeaklyUp` compatibility、`Reachability` matrix、indirect connection handling。`GossipEnvelope`、dedicated heartbeat、SeedNodeProcess、pubsub、discovery、hard 項目は別 spec に分離済み。
- **Implications**: design と tasks は core/membership の state model と downing input evidence に限定し、transport、wire、decision policy を含めない。

### existing fraktor membership model

- **Context**: 既存境界を壊さずに追加できる場所を確認した。
- **Sources Consulted**: `modules/cluster-core-kernel/src/membership/node_record.rs`, `node_status.rs`, `membership_table.rs`, `membership_snapshot.rs`, `current_cluster_state.rs`, `membership_coordinator.rs`, `modules/cluster-core-kernel/src/downing_provider/failure_observation.rs`
- **Findings**: `MembershipTable` は authority key の `BTreeMap` で `NodeRecord` を保持し、`NodeStatus::Suspect` を failure observation の主要表現にしている。`CurrentClusterState` は `members` と `unreachable` を持つが、reachability matrix record は持たない。
- **Implications**: `NodeRecord` に identity/data center を追加し、reachability は `MembershipTable` に直接混ぜ込まず、独立した `ReachabilityMatrix` と snapshot を持たせる。

### Pekko reference semantics

- **Context**: `UniqueAddress` と `Reachability` の最小 semantic を確認した。
- **Sources Consulted**: `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/Member.scala`, `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/Reachability.scala`
- **Findings**: Pekko `UniqueAddress` は address + longUid で member incarnation を区別する。`Reachability` は observer / subject / status / version の records を持ち、reachable は既定値として不要 record を prune する。集約状態は terminated が unreachable より強い。
- **Implications**: fraktor-rs では `fraktor-remote-core-rs::address::UniqueAddress` を core dependency として使う案を採用し、matrix は no_std + alloc の immutable-ish update contract として設計する。

### upstream and downstream spec boundaries

- **Context**: batch roadmap 上の dependency direction を確認した。
- **Sources Consulted**: `.kiro/steering/roadmap.md`, `.kiro/specs/cluster-active-compatibility-baseline/*`, `openspec/specs/cluster-provider-boundary/spec.md`
- **Findings**: この spec は `cluster-active-compatibility-baseline` の後続で、gossip/heartbeat、downing SBR、pubsub、serialization の upstream になる。provider boundary は topology input を cluster core へ渡すが、reachability decision は持たない。
- **Implications**: Downing へ渡すのは `FailureObservation` と indirect evidence までに留め、SBR strategy や lease majority は後続 spec へ残す。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Remote `UniqueAddress` reuse | `fraktor-remote-core-rs::address::UniqueAddress` を membership identity に使う | 既存の Pekko Artery identity と一致し、重複型を避けられる | cluster-core の production dependency が増える | 採用 |
| Cluster-local identity type | cluster-core 内に独自 `NodeIdentity` を定義する | dependency を増やさない | remote handshake identity と二重化し、serialization spec で再変換が必要 | 不採用 |
| Authority-only continuation | 既存 `authority` + `node_id` を維持する | 変更が小さい | 同じ address の再 incarnation を区別できず、gap を閉じられない | 不採用 |

## Design Decisions

### Decision: membership identity は address + uid を authoritative key にする

- **Context**: 現在は authority 文字列が主 key だが、同じ host/port を再利用する node incarnation を区別できない。
- **Alternatives Considered**:
  1. authority を key として維持する。
  2. cluster-local identity type を追加する。
  3. remote-core `UniqueAddress` を使う。
- **Selected Approach**: `UniqueAddress` を `NodeRecord` と reachability record の identity として扱い、authority は表示・互換・lookup 補助へ降格する。
- **Rationale**: 既存 remote core が Pekko semantics を持ち、cluster と remote の境界で identity 変換を増やさずに済む。
- **Trade-offs**: `fraktor-cluster-core-kernel-rs` から `fraktor-remote-core-rs` への production dependency 追加が必要になる。
- **Follow-up**: no_std check で dependency feature が std を引き込まないことを確認する。

### Decision: reachability は membership status から独立した matrix として扱う

- **Context**: `NodeStatus::Suspect` だけでは observer ごとの観測差や partial connectivity を表現できない。
- **Alternatives Considered**:
  1. `NodeStatus` に observer 情報を追加する。
  2. `MembershipTable` の side map として reachability records を持つ。
  3. 独立した `ReachabilityMatrix` を定義する。
- **Selected Approach**: `ReachabilityMatrix` が observer / subject / status / version records を所有し、membership snapshot は matrix snapshot を参照または同梱する。
- **Rationale**: status transition と reachability evidence の責務を分け、downing や pubsub が同じ matrix を参照できる。
- **Trade-offs**: `MembershipCoordinator` は failure detector event から status と matrix の両方を更新する必要がある。
- **Follow-up**: full gossip merge は後続 spec に残し、matrix 自体の local update / bounded merge contract だけを実装する。

### Decision: indirect connection handling は decision ではなく evidence に限定する

- **Context**: Pekko SBR の indirect connection handling は downing decision と密接だが、この spec の scope は membership core である。
- **Alternatives Considered**:
  1. downing strategy 判定まで含める。
  2. partial connectivity を membership event としてだけ出す。
  3. downing input に渡せる evidence 型を定義する。
- **Selected Approach**: `IndirectConnectionEvidence` を membership/reachability から生成し、downing boundary は evidence を受け取れるが判定しない。
- **Rationale**: 後続 `cluster-downing-sbr-decision-model` が policy を所有でき、membership spec が SBR を吸収しない。
- **Trade-offs**: この spec 単体では downing behavior は変わらない。
- **Follow-up**: downstream design で evidence consumption と SBR strategy を接続する。

## Risks & Mitigations

- remote-core dependency が std を引き込む — Cargo feature と no_std check を task に含める。
- `WeaklyUp` が activation/routing の active 判定を壊す — `NodeStatus` の helper を明示し、caller が暫定参加を判定できる contract にする。
- reachability matrix が gossip merge を先取りする — local matrix update と snapshot contract に限定し、full gossip merge / wire は後続 spec に残す。
- indirect evidence が SBR decision を含み始める — evidence 型は observation と classification だけを持ち、downing decision enum を生成しない。

## References

- `docs/gap-analysis/cluster-gap-analysis.md`
- `.kiro/steering/roadmap.md`
- `.kiro/specs/cluster-active-compatibility-baseline/design.md`
- `modules/remote-core/src/address/unique_address.rs`
- `modules/cluster-core-kernel/src/membership/node_record.rs`
- `modules/cluster-core-kernel/src/membership/membership_table.rs`
- `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/Member.scala`
- `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/Reachability.scala`
