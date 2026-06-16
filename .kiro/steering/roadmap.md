# Roadmap

## Overview

`docs/gap-analysis/cluster-gap-analysis.md` の `Active comparison follow-up` を、実装・仕様化しやすい責務境界に分けて進める。対象は `trivial / easy`、`medium`、`hard` に列挙された active 比較項目であり、`Deferred Pekko concepts` は含めない。

この roadmap は Pekko parity 全体を一括で追うものではなく、現在の fraktor-rs cluster runtime に隣接する active comparison follow-up を、既存の Grain runtime / provider boundary / module organization と衝突しない単位へ分解するための作業索引である。

**Phase 2 (2026-06-11)**: gap analysis の再検証（parity 分母 151 概念への再構成）で確定した「実装優先度 Phase 1（trivial / easy、24 項目）」を、責務境界で 6 つの新規 spec + 既存 spec 更新 1 件 + 直接実装 2 件に分解した。前提 SPI が必要な項目（extractor 実装群、基本 CRDT 型群）は、依存する medium 項目（extractor SPI、ReplicatedData 基底 SPI）を同じ spec に前倒しで束ね、配線されない型を作らない。

## Approach Decision

- **Chosen**: active follow-up を7つの spec に分け、依存順に仕様化する。
- **Why**: follow-up は provider/config、membership、gossip/heartbeat、downing、discovery、pubsub、serialization に責務が分かれる。単一 spec にすると 20+ task になり、review scope と PR scope が大きくなりすぎる。
- **Rejected alternatives**: 難易度別に3 spec へ分ける案は、同じ責務が easy/medium/hard にまたがって混ざるため、実装境界が曖昧になる。全項目を直接実装する案は、runtime contract の変更が多く OpenSpec と gap analysis の追跡性が落ちる。

**Phase 2 の判断**: gap analysis Phase 1（trivial/easy 24 項目）も同じ責務分割方針を踏襲し、6 spec に分ける。難易度・層別の粗い 3 spec 案は Phase 1 と同じ理由で却下。1 項目 1 spec の 24 分割案は review/PR の固定費が過大なため却下。極小項目（routee std 配線、mediator 全体 Count）は spec を立てず直接実装とし、既存完了 spec の自然な拡張（Multi-DC failure detector 設定）は既存 spec の更新として扱う。

## Scope

- **In**:
  - (Phase 1) `Active comparison follow-up: trivial / easy`、`medium`、`hard` にある項目のうち、現在の cluster roadmap と矛盾しない comparison-driven runtime contract。
  - (Phase 2) gap analysis「実装優先度 Phase 1（trivial/easy、24 項目）」: membership イベント表層、singleton 設定契約、typed entity facade、extractor 契約 + 実装群、sharding health / join compat、基本 CRDT 型群。runtime 基盤（singleton manager、Replicator）の前提となる「契約・型・検証」までを In とする。
- **Out**: Cluster Client（Pekko 本体で全面 deprecated）、Receptionist 実装、Cluster Singleton の runtime（manager / proxy / handover election）、Distributed Data の Replicator runtime と OR/LWW 系 CRDT、sharding の rebalance / remembered entities / delivery controllers、JVM 固有機能、完全な Akka/Pekko 互換 migration layer。これらは gap analysis の Phase 2-3（medium / hard）として別フェーズで扱う。

## Constraints

- `*-core` は `no_std` 境界を維持し、Tokio・network I/O・host lifecycle は `*-adaptor-std` に置く。
- 既存の Grain runtime 方向性を優先し、Pekko public API parity を現在の cluster roadmap として扱わない。
- `docs/gap-analysis/cluster-gap-analysis.md` は comparison evidence として更新し、実装契約は spec / tests / showcases で証明する。
- 参照実装に寄せる場合も、Rust の型境界、crate boundary、port-and-adapter 方針を優先する。

## Boundary Strategy

- **Why this split**: 各 spec を module boundary と review scope に近い単位へ分けることで、membership の基礎 contract、transport/wire、downing decision、pubsub protocol を独立して進められる。
- **Shared seams to watch**: `membership` と `downing_provider`、`membership` と `pub_sub`、`cluster-adaptor-std` と `remote-adaptor-std`、actor-core serialization boundary、provider lifecycle と topology input。
- **Phase 2 で追加の seam**: `extension`（config validation / join compatibility）と singleton / sharding 設定契約、`grain` と `cluster-core-typed`（typed facade）、新設 `ddata` モジュールと membership 用 `VectorClock`（混同しないこと — CRDT 用 `VersionVector` は別物として Phase 3 で扱う）。

## Specs (dependency order)

- [x] cluster-active-compatibility-baseline -- trivial/easy 項目を config/path/provider/lifecycle の互換 baseline として整理する。Dependencies: none
- [x] cluster-membership-reachability-model -- UniqueAddress、data center、WeaklyUp、Reachability、indirect connection handling の core membership model を定義する。Dependencies: cluster-active-compatibility-baseline
- [x] cluster-gossip-heartbeat-protocol -- GossipEnvelope、cluster heartbeat、full Gossip merge/tombstone/seen digest、CrossDcClusterHeartbeat を membership + logical transport handoff 境界で定義する。Dependencies: cluster-membership-reachability-model
- [x] cluster-downing-sbr-decision-model -- SplitBrainResolver、DowningStrategy、lease-based majority を downing decision model として定義する。Dependencies: cluster-active-compatibility-baseline, cluster-membership-reachability-model
- [x] cluster-discovery-provider-interop -- SeedNodeProcess と generic discovery adapter を provider boundary に追加する。Dependencies: cluster-active-compatibility-baseline
- [x] cluster-pubsub-mediator-protocol -- DistributedPubSubMediator、settings、Send / SendToAll path semantics、topic registry gossip / delta collection を定義する。Dependencies: cluster-membership-reachability-model, cluster-gossip-heartbeat-protocol
- [x] cluster-message-serialization-contract -- cluster message serializer contract を std/wire と actor-core serialization の境界で定義する。Dependencies: cluster-gossip-heartbeat-protocol, cluster-pubsub-mediator-protocol
- [x] cluster-membership-event-surface -- Member ordering 公開契約、shutdown 進行 / DC reachability の membership イベント variant、cluster lifecycle 構造化 tracing field 契約を定義する。Dependencies: cluster-membership-reachability-model（2026-06-11 完了、PR #1982）
- [x] cluster-singleton-settings-contract -- Cluster Singleton の設定契約（classic/typed settings、setup、stuck 検知契約）を config validation / join compatibility に接続して定義する。Dependencies: cluster-active-compatibility-baseline
- [x] cluster-grain-typed-entity-facade -- GrainKey / GrainRef の typed wrapper（EntityTypeKey / EntityRef 相当）と typed ActorSystem setup 統合を定義する。Dependencies: none
- [x] cluster-sharding-extractor-contract -- ShardingEnvelope / ShardingMessageExtractor SPI と HashCode / Murmur2 標準実装群を定義する。Dependencies: cluster-grain-typed-entity-facade（2026-06-13 完了、PR #1990）
- [x] cluster-sharding-health-and-join-compat -- grain/placement の readiness check（core 完結の純粋判定 + 公開アクセサ）と sharding join compatibility の除外キー整備（required key の追加は config 所有化とともに後続スペックへ委譲）を定義する。Dependencies: cluster-active-compatibility-baseline（2026-06-13 完了、PR #1993）
- [x] cluster-ddata-core-types -- ReplicatedData 基底 SPI、Key、SelfUniqueAddress、基本 CRDT（Flag / GCounter / PNCounter / PNCounterMap）、consistency levels、補助 protocol 型を新設 ddata モジュールに定義する。Dependencies: none

## Existing Spec Updates

- [x] configure-cluster-failure-detector -- Multi-DC failure detector 設定 namespace（`CrossDcFailureDetectorSettings` / `MultiDataCenter` 相当）を既存の FailureDetectorConfig / validation / join compatibility 構造に追加する。Dependencies: none

## Direct Implementation Candidates

- [x] cluster-router routee 更新の std 配線 -- core policy（`ClusterRouterPool::update_from_members`）は実装済みで、ClusterEvent 購読 → routee 更新の std 配線 1 本 + 統合テストのみ。spec を立てる規模ではない
- [x] pubsub mediator 全体 `Count` query -- `MediatorQuery` への variant 1 つ追加 + 集計 + テストで閉じる trivial 変更
