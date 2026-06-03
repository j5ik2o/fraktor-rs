# Brief: cluster-downing-sbr-decision-model

## Problem

downing / Split Brain Resolver 系 follow-up は provider hook から hard な decision model まで幅がある。membership reachability が整った後、core downing decision と std/provider integration を明確に分ける必要がある。

## Current State

`cluster-core-kernel/src/downing_provider` には downing input / decision / compatibility / split brain resolver settings / strategy の基礎がある。gap analysis では `SplitBrainResolverProvider`、`SplitBrainResolver`、`DowningStrategy` / decision model、lease-based majority が active follow-up として残っている。

## Desired Outcome

downing decision model が reachability evidence と membership snapshot を入力として扱い、SBR strategy と lease majority を core contract として評価できる。std/provider は lifecycle と lease backend integration を担当し、decision semantics を所有しない。

## Approach

baseline spec の provider hook を前提に、core/downing_provider で strategy input/output、decision trace、SBR compatibility、lease majority port を定義する。lease の host-specific 実装は std adaptor 側の port implementation とする。

## Scope

- **In**: `SplitBrainResolver`、`DowningStrategy` / decision model、lease-based majority、provider-facing SBR integration。
- **Out**: full discovery backend、Gossip merge、CrossDc heartbeat、Cluster Singleton。

## Boundary Candidates

- core/downing_provider: strategy、input、decision、trace、lease majority port
- std/provider: provider lifecycle と lease backend binding
- core/membership: reachability / membership snapshot input

## Out of Boundary

- Membership reachability matrix 自体の定義
- TCP heartbeat scheduling
- Pekko Split Brain Resolver public API 完全互換

## Upstream / Downstream

- **Upstream**: `cluster-membership-reachability-model`, `cluster-active-compatibility-baseline`
- **Downstream**: provider-driven downing behavior、future operational docs

## Existing Spec Touchpoints

- **Extends**: `openspec/specs/cluster-provider-boundary`
- **Adjacent**: `openspec/specs/cluster-grain-runtime-operational-contract`

## Constraints

downing は Grain runtime の topology input boundary として扱う。Cluster Sharding rebalance、remembered entity recovery、in-flight draining はこの spec に含めない。
