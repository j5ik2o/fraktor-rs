# Roadmap

## Overview

`docs/gap-analysis/cluster-gap-analysis.md` の `Active comparison follow-up` を、実装・仕様化しやすい責務境界に分けて進める。対象は `trivial / easy`、`medium`、`hard` に列挙された active 比較項目であり、`Deferred Pekko concepts` は含めない。

この roadmap は Pekko parity 全体を一括で追うものではなく、現在の fraktor-rs cluster runtime に隣接する active comparison follow-up を、既存の Grain runtime / provider boundary / module organization と衝突しない単位へ分解するための作業索引である。

## Approach Decision

- **Chosen**: active follow-up を7つの spec に分け、依存順に仕様化する。
- **Why**: follow-up は provider/config、membership、gossip/heartbeat、downing、discovery、pubsub、serialization に責務が分かれる。単一 spec にすると 20+ task になり、review scope と PR scope が大きくなりすぎる。
- **Rejected alternatives**: 難易度別に3 spec へ分ける案は、同じ責務が easy/medium/hard にまたがって混ざるため、実装境界が曖昧になる。全項目を直接実装する案は、runtime contract の変更が多く OpenSpec と gap analysis の追跡性が落ちる。

## Scope

- **In**: `Active comparison follow-up: trivial / easy`、`medium`、`hard` にある項目のうち、現在の cluster roadmap と矛盾しない comparison-driven runtime contract。
- **Out**: `Deferred Pekko concepts` に列挙された Cluster Singleton、Cluster Client、Receptionist、Distributed Data / CRDT、Cluster Sharding public API parity、JVM 固有機能、完全な Akka/Pekko 互換 migration layer。

## Constraints

- `*-core` は `no_std` 境界を維持し、Tokio・network I/O・host lifecycle は `*-adaptor-std` に置く。
- 既存の Grain runtime 方向性を優先し、Pekko public API parity を現在の cluster roadmap として扱わない。
- `docs/gap-analysis/cluster-gap-analysis.md` は comparison evidence として更新し、実装契約は spec / tests / showcases で証明する。
- 参照実装に寄せる場合も、Rust の型境界、crate boundary、port-and-adapter 方針を優先する。

## Boundary Strategy

- **Why this split**: 各 spec を module boundary と review scope に近い単位へ分けることで、membership の基礎 contract、transport/wire、downing decision、pubsub protocol を独立して進められる。
- **Shared seams to watch**: `membership` と `downing_provider`、`membership` と `pub_sub`、`cluster-adaptor-std` と `remote-adaptor-std`、actor-core serialization boundary、provider lifecycle と topology input。

## Specs (dependency order)

- [x] cluster-active-compatibility-baseline -- trivial/easy 項目を config/path/provider/lifecycle の互換 baseline として整理する。Dependencies: none
- [x] cluster-membership-reachability-model -- UniqueAddress、data center、WeaklyUp、Reachability、indirect connection handling の core membership model を定義する。Dependencies: cluster-active-compatibility-baseline
- [x] cluster-gossip-heartbeat-protocol -- GossipEnvelope、cluster heartbeat、full Gossip merge/tombstone/seen digest、CrossDcClusterHeartbeat を membership + logical transport handoff 境界で定義する。Dependencies: cluster-membership-reachability-model
- [x] cluster-downing-sbr-decision-model -- SplitBrainResolver、DowningStrategy、lease-based majority を downing decision model として定義する。Dependencies: cluster-active-compatibility-baseline, cluster-membership-reachability-model
- [x] cluster-discovery-provider-interop -- SeedNodeProcess と generic discovery adapter を provider boundary に追加する。Dependencies: cluster-active-compatibility-baseline
- [x] cluster-pubsub-mediator-protocol -- DistributedPubSubMediator、settings、Send / SendToAll path semantics、topic registry gossip / delta collection を定義する。Dependencies: cluster-membership-reachability-model, cluster-gossip-heartbeat-protocol
- [x] cluster-message-serialization-contract -- cluster message serializer contract を std/wire と actor-core serialization の境界で定義する。Dependencies: cluster-gossip-heartbeat-protocol, cluster-pubsub-mediator-protocol
