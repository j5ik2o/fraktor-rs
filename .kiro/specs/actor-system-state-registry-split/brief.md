# Brief: actor-system-state-registry-split

## Problem

`SystemState` / `SystemStateShared` は dispatcher registry、cell table、guardian、serialization、remote hook、scheduler などをまとめて保持しており、EventBus や mailbox workstream の変更が無関係な system state に波及しやすい。actor gap closure の Phase 2 は mailbox / EventBus / shutdown に触れるため、registry 境界を先に分ける必要がある。

## Current State

`modules/actor-core-kernel/src/system/state.rs` と周辺 shared 型は複数 subsystem の状態と accessor を同居させている。既存の `system/registries.rs` などの分離単位はあるが、system state 自体は束ね役を超えて具象責務を持つ。

## Desired Outcome

SystemState は subsystem registry の束ね役に縮小される。dispatcher / mailbox / event / guardian / serialization / remote / scheduler などは責務ごとの private struct または module に分離され、後続の EventBus / mailbox / CoordinatedShutdown 変更が局所化される。

## Approach

外部 API を変えずに internal registry を抽出し、既存 accessor は段階的に新 registry へ委譲する。shared wrapper は project-defined `Shared*` / `ArcShared` パターンに合わせ、直接の `Arc` / `Mutex` 追加を避ける。

## Scope

- **In**: SystemState / SystemStateShared の subsystem registry 分離、既存 accessor の委譲化、対象 unit / integration test の維持
- **Out**: mailbox resolution の新仕様、EventBus trait 族の導入、CoordinatedShutdown task variant の導入、remote / serialization の public behavior 変更

## Boundary Candidates

- mailbox / dispatcher registry
- event stream / logging registry
- guardian / cell table registry
- serialization / remote hook registry
- scheduler / shutdown coordination state

## Out of Boundary

- ActorCell facet 分割
- typed system の facade 分離
- public re-export audit

## Upstream / Downstream

- **Upstream**: 既存 `actor-core-kernel` system state と shared wrapper
- **Downstream**: actor-eventbus-classification-contract、actor-mailbox-resolution-contract、actor-coordinated-shutdown-task-variants

## Existing Spec Touchpoints

- **Extends**: なし
- **Adjacent**: actor-cell-facet-structure、actor-kernel-public-surface-audit

## Constraints

`actor-core-kernel` の `no_std` 境界を維持する。shared state の操作は closure-based API を優先し、read-then-act を増やさない。構造整理 spec なので observable behavior を変えない。
