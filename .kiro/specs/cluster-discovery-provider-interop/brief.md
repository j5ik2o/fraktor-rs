# Brief: cluster-discovery-provider-interop

## Problem

active medium の provider/discovery 項目は、SeedNodeProcess と generic discovery adapter に分かれている。これらを provider boundary として整理しないと、std adaptor が discovery policy と topology policy を混ぜて持ちやすい。

## Current State

`cluster-provider-boundary` は provider input を topology input に正規化する方針を持ち、`cluster-adaptor-std` には local provider と AWS ECS provider がある。gap analysis では `SeedNodeProcess` と generic discovery adapter が未整理の active follow-up として残っている。

## Desired Outcome

SeedNodeProcess と generic discovery adapter が provider boundary の一部として定義され、core placement / membership は provider-specific discovery details に依存しない。std adaptor は discovery source を topology input に変換するだけに留まる。

## Approach

core に provider-neutral discovery result / seed node process contract を置き、std adaptor に discovery backend bridge を置く。既存 local/static/AWS ECS provider と衝突しないよう、provider lifecycle と topology publication の責務を明確にする。

## Scope

- **In**: `SeedNodeProcess`、generic discovery adapter、provider lifecycle と topology input への変換。
- **Out**: SBR decision model、full Gossip protocol、pubsub mediator、AWS ECS 固有機能の拡張。

## Boundary Candidates

- core/cluster_provider: provider-neutral seed / discovery contract
- std/provider: discovery backend adapter
- topology input: normalized member update boundary

## Out of Boundary

- Discovery backend の網羅実装
- Provider-specific retry / auth / cloud API policy
- Membership gossip merge

## Upstream / Downstream

- **Upstream**: `cluster-active-compatibility-baseline`
- **Downstream**: membership bootstrap、std adaptor provider integrations

## Existing Spec Touchpoints

- **Extends**: `openspec/specs/cluster-provider-boundary`
- **Adjacent**: `openspec/specs/cluster-grain-runtime-operational-contract`

## Constraints

std adaptor は provider-specific discovery details を core placement logic に漏らさない。generic adapter は拡張点であり、特定 cloud provider の完全実装をこの spec で所有しない。
