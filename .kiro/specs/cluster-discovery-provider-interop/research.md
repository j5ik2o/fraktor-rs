# 調査と設計判断（Research & Design Decisions）

## Summary（概要）

- **Feature**: `cluster-discovery-provider-interop`
- **Discovery Scope**: Extension
- **Key Findings**:
  - 既存の `LocalClusterProvider::with_seed_nodes` は seed list を保持できるが、SeedNodeProcess として lifecycle / failure / topology input 変換を独立契約化していない。
  - `cluster-provider-boundary` は provider input を topology input へ正規化する方針をすでに定義しているため、この仕様はその境界を discovery provider interop に拡張する。
  - `cluster-adaptor-std` には AWS ECS polling と remoting lifecycle subscription があるが、generic discovery backend を差し替える provider-neutral adapter contract は限定的である。

## Research Log（調査ログ）

### gap analysis scope

- **Context**: active medium follow-up から provider/discovery interop に属する項目だけを抽出した。
- **Sources Consulted**: `docs/gap-analysis/cluster-gap-analysis.md`
- **Findings**: 対象は `SeedNodeProcess` と generic discovery adapter。`UniqueAddress`、data center、`WeaklyUp`、Reachability、GossipEnvelope、heartbeat、indirect connection、pubsub は別 spec に分離されている。
- **Implications**: requirements と tasks は seed / discovery / provider lifecycle / topology input 変換に限定する。

### existing provider boundary

- **Context**: 既存の provider contract と topology publication pattern を確認した。
- **Sources Consulted**: `modules/cluster-core-kernel/src/cluster_provider.rs`, `modules/cluster-core-kernel/src/cluster_provider/local_cluster_provider_generic.rs`, `modules/cluster-core-kernel/src/topology/topology_update.rs`, `openspec/specs/cluster-provider-boundary/spec.md`
- **Findings**: `ClusterProvider` は start / join / leave / down / shutdown を定義する。`LocalClusterProvider` は join / leave を `ClusterEvent::TopologyUpdated` に変換し、block list を topology update に含める。provider boundary spec は discovery details を placement logic の外側に残すことを要求している。
- **Implications**: SeedNodeProcess と generic discovery adapter は new membership model を所有せず、既存 topology update contract へ入力を渡す。

### std adaptor discovery surfaces

- **Context**: std 側にある discovery / lifecycle 実装との接続点を確認した。
- **Sources Consulted**: `modules/cluster-adaptor-std/src/cluster_provider/local_cluster_provider_ext.rs`, `modules/cluster-adaptor-std/src/cluster_provider/aws_ecs_cluster_provider.rs`, `modules/cluster-adaptor-std/src/cluster_provider/ecs_task_discovery.rs`
- **Findings**: remoting lifecycle bridge は subscription lifetime と weak provider handle を持つ。AWS ECS provider は polling task を provider lifecycle が所有し、ECS task discovery result を authority string に変換して topology update を publish している。
- **Implications**: generic discovery adapter は std adaptor に置き、polling/subscription lifecycle と result normalization を provider lifecycle の一部にする。

### upstream and downstream boundaries

- **Context**: adjacent specs との重複を避けるため、dependency direction を確認した。
- **Sources Consulted**: `.kiro/steering/roadmap.md`, `.kiro/specs/cluster-active-compatibility-baseline/design.md`, `openspec/specs/cluster-grain-runtime-operational-contract/spec.md`
- **Findings**: upstream は compatibility baseline。downstream membership / gossip / downing / pubsub / serialization は provider-neutral topology input を受け取る側であり、discovery backend の詳細を所有しない。
- **Implications**: design は downstream revalidation triggers を明示し、provider/discovery spec が membership semantics を吸収しないようにする。

## Architecture Pattern Evaluation（アーキテクチャパターン評価）

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Provider boundary extension | core に provider-neutral result / SeedNodeProcess contract、std に discovery adapter bridge を追加する | 既存 `ClusterProvider` と topology contract に沿う | core と std の責務を混ぜると境界が崩れる | 採用 |
| AWS ECS provider にだけ統合 | 既存 ECS polling を拡張して discovery adapter 相当にする | 実装量は小さい | generic backend 差し替えができず、ECS 固有 metadata が漏れやすい | 不採用 |
| membership coordinator に discovery を統合 | discovery から直接 membership state を更新する | 起動 path は短い | reachability/gossip と discovery が結合する | 不採用 |

## Design Decisions（設計判断）

### Decision: discovery result は core contract、backend execution は std adaptor に置く

- **Context**: core placement / membership は provider-specific discovery details に依存してはならない。
- **Alternatives Considered**:
  1. discovery backend trait を core へ置き、backend 実行も core contract に含める。
  2. provider-neutral discovery result と seed process contract を core に置き、backend execution は std adaptor に置く。
- **Selected Approach**: core は `DiscoveryResult`、`DiscoveredAuthority`、SeedNodeProcess の状態入力 contract を持つ。std は backend polling/subscription と topology input conversion を持つ。
- **Rationale**: `*-core` の `no_std` 境界を守り、Tokio / AWS SDK / network I/O を std adaptor に閉じ込められる。
- **Trade-offs**: std adaptor 側に bridge 実装が必要になるが、backend 差し替えは provider lifecycle の外側に漏れない。
- **Follow-up**: implementation で `no_std` check と std targeted tests を分けて実行する。

### Decision: SeedNodeProcess は join orchestration の入口に限定する

- **Context**: Pekko の `SeedNodeProcess` は active join orchestration の比較対象だが、full membership/gossip は別 spec にある。
- **Alternatives Considered**:
  1. SeedNodeProcess から membership status と gossip seen set まで扱う。
  2. seed source の取得、dedup、self filtering、join input publication だけを扱う。
- **Selected Approach**: SeedNodeProcess は startup / refresh / shutdown lifecycle と topology join input の生成に限定する。
- **Rationale**: membership reachability と gossip heartbeat を downstream spec に残せる。
- **Trade-offs**: join 成功後の membership convergence はこの仕様単体では完結しない。
- **Follow-up**: `cluster-membership-reachability-model` と `cluster-gossip-heartbeat-protocol` が convergence semantics を所有する。

### Decision: gap analysis 更新は2項目だけに限定する

- **Context**: active medium には discovery 以外の項目が混在している。
- **Alternatives Considered**:
  1. provider interop の実装時に active medium の周辺項目もまとめて更新する。
  2. `SeedNodeProcess` と generic discovery adapter の evidence だけを更新する。
- **Selected Approach**: gap analysis 更新は2項目だけに限定する。
- **Rationale**: roadmap の spec boundary を維持し、Deferred Pekko concepts を誤って完了扱いにしない。
- **Trade-offs**: active medium 全体の完了率は別 spec の完了まで残る。
- **Follow-up**: downstream spec 実装時に各自の evidence を更新する。

## Risks & Mitigations（リスクと緩和策）

- discovery adapter が membership semantics を持ち始める — topology input 変換までを boundary とし、reachability / WeaklyUp / gossip は out of boundary に固定する。
- generic adapter が過剰抽象化になる — current scope は seed/source result の最小 contract と std bridge に限定し、backend 固有 feature は実装しない。
- provider lifecycle が task leak を起こす — shutdown と weak provider handle の acceptance / tasks を明示する。
- AWS ECS 既存 behavior を壊す — generic adapter は既存 ECS provider を置き換えず、adapter bridge への移行可能な contract として追加する。

## References（参照）

- `docs/gap-analysis/cluster-gap-analysis.md`
- `.kiro/steering/roadmap.md`
- `.kiro/specs/cluster-active-compatibility-baseline/design.md`
- `openspec/specs/cluster-provider-boundary/spec.md`
- `modules/cluster-core-kernel/src/cluster_provider.rs`
- `modules/cluster-core-kernel/src/cluster_provider/local_cluster_provider_generic.rs`
- `modules/cluster-adaptor-std/src/cluster_provider/local_cluster_provider_ext.rs`
- `modules/cluster-adaptor-std/src/cluster_provider/aws_ecs_cluster_provider.rs`
