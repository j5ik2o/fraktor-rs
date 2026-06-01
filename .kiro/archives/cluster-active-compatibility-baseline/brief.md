# Brief: cluster-active-compatibility-baseline

## Problem

cluster gap analysis の active trivial/easy 項目には、runtime の大きな設計変更ではないが、Pekko comparison 上の互換 surface として未整理の config / path / provider / lifecycle 項目が残っている。これらを直接ばらばらに実装すると、後続の membership / downing / discovery work の前提が曖昧になる。

## Current State

`cluster-core-kernel` と `cluster-adaptor-std` には provider boundary、cluster router、remote delivery、transport lifecycle に関する既存実装と OpenSpec がある。一方で `SplitBrainResolverProvider`、config compatibility full key set、`remotePathOf`、transport lifecycle bridge retention は active follow-up として残っている。

## Desired Outcome

trivial/easy 項目が、後続 spec の前提になる compatibility baseline として整理される。実装は core/adaptor 境界を守り、後続の membership / downing / discovery work が同じ config・path・lifecycle 語彙を使える状態にする。

## Approach

config key set、remote path helper、downing provider / SBR settings compatibility metadata、transport lifecycle bridge retention をひとつの小さな baseline spec にまとめる。Pekko API 完全互換ではなく、fraktor-rs の cluster runtime が参照できる契約名と最小 surface を定義する。

## Scope

- **In**: downing provider / SBR settings compatibility metadata、config compatibility full key set、`remotePathOf`、transport lifecycle bridge retention。
- **Out**: full SplitBrainResolver 実装、DowningStrategy decision model、generic discovery backend、Gossip protocol、Cluster Sharding / Singleton parity。

## Boundary Candidates

- core/config: compatibility key set と validation
- actor-core integration: remote path calculation surface
- std/provider: provider lifecycle bridge と downing compatibility metadata

## Out of Boundary

- Lease majority や split brain decision 本体
- SeedNodeProcess と discovery polling
- serializer binary compatibility

## Upstream / Downstream

- **Upstream**: `cluster-provider-boundary`, `cluster-adaptor-std-remote-delivery`, `cluster-core-module-organization`
- **Downstream**: `cluster-membership-reachability-model`, `cluster-downing-sbr-decision-model`, `cluster-discovery-provider-interop`

## Existing Spec Touchpoints

- **Extends**: `openspec/specs/cluster-provider-boundary`, `openspec/specs/cluster-adaptor-std-remote-delivery`
- **Adjacent**: `openspec/specs/cluster-grain-runtime-operational-contract`

## Constraints

Pekko public API parity を目的にしない。`std` lifecycle は adaptor 側に置き、core は config / port / value contract に留める。
