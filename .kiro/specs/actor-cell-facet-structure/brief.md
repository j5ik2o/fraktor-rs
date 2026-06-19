# Brief: actor-cell-facet-structure

## Problem

`docs/gap-analysis/actor-gap-analysis.md` は `ActorCell` が dispatch / fault handling / DeathWatch / children / receive timeout / stash / timers / pipe を 1 ファイルに抱え、後続の actor parity work の変更面を大きくしていると指摘している。Phase 1 API gap の多くは `ActorCell` 周辺に触れるため、先に責務を分けないと小さな parity 追加でも巨大 diff になりやすい。

## Current State

`modules/actor-core-kernel/src/actor/actor_cell.rs` は複数責務をまとめた orchestrator になっている。receive timeout は `receive_timeout_state.rs` / `receive_timeout_state_shared.rs` / `actor_cell.rs` / `actor_context.rs` に分散している。既存 public behavior は高く実装済みなので、目的はふるまい追加ではなく構造整理である。

## Desired Outcome

`ActorCell` の同一型に対する `impl` を dispatch、fault handling、DeathWatch、children、receive timeout などの facet 単位へ分割し、`ActorCell` 本体は orchestration と公開境界に寄せる。receive timeout の状態遷移と cell 連携は専用 facet に集約され、既存テストは同じふるまいを保ったまま通る。

## Approach

公開 API を増やさず、同一クレート内の module / impl split と sibling test の再配置で進める。Pekko の `actor/dungeon/*` は責務分離の参照に使うが、Rust では継承 trait ではなく module と private helper の分離で表現する。

## Scope

- **In**: `ActorCell` の facet 分割、receive timeout 責務の集約、既存テストの移動または分割、後続 API work のための private helper 境界整理
- **Out**: 新しい actor public API、mailbox selection の仕様変更、EventBus の汎用化、supervision policy の意味変更

## Boundary Candidates

- dispatch facet と mailbox / dispatcher 側 contract
- fault handling facet と supervision / lifecycle contract
- DeathWatch / children facet と actor tree management
- receive timeout facet と context API

## Out of Boundary

- `SystemState` の registry split
- typed actor facade の分離
- 既存 public re-export の visibility audit

## Upstream / Downstream

- **Upstream**: 既存 `actor-core-kernel` の ActorCell / ActorContext / lifecycle / receive timeout 実装
- **Downstream**: actor-kernel-message-observability、actor-coordinated-shutdown-task-variants、将来の lifecycle / supervision parity work

## Existing Spec Touchpoints

- **Extends**: なし
- **Adjacent**: actor-kernel-message-observability、actor-kernel-public-surface-audit

## Constraints

`actor-core-kernel` の `no_std` + alloc 境界を維持する。構造変更は挙動を変えず、既存 public API の追加・削除は行わない。1 公開型 1 ファイル、sibling `_test.rs`、FQCN import rule に従う。
