# Brief: actor-kernel-public-surface-audit

## Problem

actor gap analysis は API gap が縮んだ後のボトルネックとして、kernel public surface の広さと層配置の曖昧さを挙げている。`ActorCell` / `ActorShared` / `ChildRef` などの低レベル型が public re-export され、`SystemMessage` の配置も dispatch 責務とずれているため、外部契約と internal surface の境界が読み取りにくい。

## Current State

`actor-core-kernel` は runtime 実装に必要な低レベル型を広めに公開している。`SystemMessage` は actor messaging 配下にあり、dispatch / mailbox 側が actor domain から取得する形になっている。typed facade / behavior 実装の混在も一部残る。

## Desired Outcome

actor kernel の public re-export が「利用者向け contract」と「crate internal implementation」に整理される。`SystemMessage` の責務配置が dispatch 層へ寄せられるか、少なくとも現在配置の理由が明文化される。typed receptionist setup 後に残る facade / behavior 混在も棚卸しされる。

## Approach

後続 API specs が入ったあとに public surface を棚卸しし、実際に外部から必要な型だけを公開する。pre-release 前提なので破壊的 visibility 変更は許容するが、各削除 / 移動は compile error と tests で影響範囲を確認してから行う。

## Scope

- **In**: public re-export audit、低レベル型の `pub(crate)` 化候補整理、`SystemMessage` の dispatch 層配置検討と移動、typed facade 残存混在の棚卸し、gap analysis 更新
- **Out**: 新しい actor behavior、mailbox resolution の仕様追加、EventBus trait の新設、cluster / remote API の整理

## Boundary Candidates

- external actor runtime API と internal cell / system implementation
- actor messaging と dispatch system message
- typed facade API と behavior implementation

## Out of Boundary

- ActorCell facet split の実作業
- SystemState registry split の実作業
- ReceptionistSetup の導入

## Upstream / Downstream

- **Upstream**: actor-kernel-message-observability、actor-eventbus-classification-contract、actor-mailbox-resolution-contract、actor-typed-receptionist-setup
- **Downstream**: README / docs public API cleanup、future actor parity specs

## Existing Spec Touchpoints

- **Extends**: なし
- **Adjacent**: actor-cell-facet-structure、actor-system-state-registry-split、actor-typed-receptionist-setup

## Constraints

人間の明示判断なしに `.coderabbit.yml` / `.coderabbit.yaml` は変更しない。public surface の削減は compile / tests / docs update と一体で確認する。単なる好みの rename は扱わず、gap analysis に根拠がある境界整理に限定する。
