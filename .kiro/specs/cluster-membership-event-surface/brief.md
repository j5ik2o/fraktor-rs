# Brief: cluster-membership-event-surface

## Problem

cluster membership の観測面（イベント・順序・tracing）に Pekko parity の欠けがある。上位コンポーネント（router、将来の singleton、coordinated shutdown 協調、運用監視）が、member の age 順序、graceful shutdown の進行、data center 単位の reachability 変化を観測できない。gap analysis（`docs/gap-analysis/cluster-gap-analysis.md` カテゴリ1/2）で easy gap として特定済み。

## Current State

- `NodeStatus` に `PreparingForShutdown` / `ReadyForShutdown` バリアントと `MembershipCoordinator` / `GossipStateModel` の状態遷移は実装済みだが、`MembershipEvent`（現状 `Joined` / `Left` / `MarkedSuspect` / `AuthorityConflict`）に shutdown 進行を通知する variant がない。
- `oldest_authority` は `membership_coordinator.rs` のプライベート関数として存在するのみで、Pekko `Member.ordering` / `Member.ageOrdering` に相当する member 順序の公開契約がない。
- `CrossDcHeartbeatEvidence` はあるが、Pekko `UnreachableDataCenter` / `ReachableDataCenter` に相当する DC 単位の reachability イベント型と発行経路がない。
- Pekko `ClusterLogMarker` に相当する cluster lifecycle 専用の構造化 tracing field 契約がない。

## Desired Outcome

- member の deterministic ordering と age ordering が公開 API として提供され、SBR の KeepOldest や将来の singleton oldest-election が同じ契約を参照できる。
- shutdown 進行（`MemberPreparingForShutdown` / `MemberReadyForShutdown` 相当）と DC 単位 reachability（`UnreachableDataCenter` / `ReachableDataCenter` 相当）が `MembershipEvent`（または隣接イベント型）の variant として発行され、event stream 購読者が観測できる。
- cluster lifecycle の主要遷移（join / up / leave / down / shutdown 進行 / DC reachability）に対する構造化 tracing field 契約が定義され、tracing 出力が機械的に解析可能になる。

## Approach

既存の `MembershipCoordinator` / `MembershipTable` / `ReachabilityMatrix` / `CrossDcHeartbeat` が保持する情報を、新しい公開契約（ordering API、イベント variant、tracing field 定義）として表面化する。状態機械そのものは変更せず、観測面だけを追加する。

## Scope

- **In**:
  - `NodeRecord` / membership snapshot に対する ordering / age ordering の公開契約
  - shutdown 進行イベント variant と発行経路
  - DC 単位 reachability イベント variant と発行経路（`CrossDcHeartbeat` evidence からの導出）
  - cluster lifecycle 構造化 tracing field 契約（`ClusterLogMarker` 相当）
- **Out**:
  - `prepareForFullClusterShutdown` command path（gap analysis Phase 2 / medium）
  - downing 判断や SBR runtime loop（Phase 3）
  - singleton の oldest-election 実装（Phase 3）

## Boundary Candidates

- ordering 契約（pure な比較関数 / newtype）と イベント発行経路（coordinator 変更）の分離
- DC reachability の「evidence → イベント導出」と membership 本体の分離

## Out of Boundary

- gossip / heartbeat protocol 本体の変更
- イベント payload の wire serialization（cluster-message-serialization-contract の領域）

## Upstream / Downstream

- **Upstream**: cluster-membership-reachability-model（完了済み）、cluster-gossip-heartbeat-protocol（完了済み）
- **Downstream**: cluster-singleton 系 runtime（Phase 3、oldest-election が ordering 契約を使う）、full cluster shutdown command path（Phase 2）、cluster-router routee 更新 std 配線（direct implementation）

## Existing Spec Touchpoints

- **Extends**: cluster-membership-reachability-model の観測面を拡張する（spec 自体は新規）
- **Adjacent**: cluster-gossip-heartbeat-protocol（CrossDcHeartbeat evidence を読むが変更しない）、cluster-downing-sbr-decision-model（KeepOldest の oldest 判定と契約を揃える）

## Constraints

- `cluster-core-kernel` は `no_std` を維持。tracing field 契約は core では型 / 定数として定義し、実際の tracing 出力は呼び出し側（std 層）の責務とする。
- 既存の `MembershipEvent` 購読者を壊す変更は許容される（pre-release、後方互換不要）が、variant 追加時は網羅 match の修正範囲を design で明示すること。
