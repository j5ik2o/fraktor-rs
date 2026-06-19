# Brief: actor-typed-receptionist-setup

## Problem

typed layer は receptionist API とローカル behavior 実装を持つが、Pekko `ReceptionistSetup` 相当の差し替え契約がない。clustered receptionist などの将来実装を入れる場合、現在の固定インストール経路では typed system 生成時に receptionist 実装を差し替えられない。

## Current State

`actor-core-typed` には `Receptionist` extension、`ServiceKey`、`Register` / `Deregister` / `Subscribe` / `Find`、`Listing` などの surface が存在する。`system.rs` の install path はローカル receptionist を固定的に組み立てる。`receptionist/extension.rs` は extension API と behavior 実装が同居している。

## Desired Outcome

typed system setup に `ReceptionistSetup` 相当の差し替え契約が追加され、ローカル receptionist を default として維持しつつ、利用者または後続 cluster work が代替 receptionist factory を渡せる。extension API、behavior 実装、内部 sender はファイル境界で分離される。

## Approach

Pekko の `ReceptionistSetup` を参照し、Rust では typed system setup に差し込む小さな factory / installer contract として表現する。clustered receptionist 実装は作らず、local receptionist の current behavior を default setup として保持する。

## Scope

- **In**: `ReceptionistSetup` 相当の setup 型、typed system install path の default / custom 分岐、receptionist extension API と behavior 実装の分離、setup tests
- **Out**: clustered receptionist runtime、cluster membership 連携、receptionist wire protocol、typed public API の大規模再設計

## Boundary Candidates

- setup contract と receptionist behavior 実装
- local receptionist default と custom receptionist factory
- typed facade と internal sender / adapter implementation

## Out of Boundary

- ActorCell / SystemState の構造整理
- EventBus generic classification
- cluster receptionist 実装

## Upstream / Downstream

- **Upstream**: 既存 `actor-core-typed` receptionist / typed system setup
- **Downstream**: actor-kernel-public-surface-audit、将来の cluster receptionist spec

## Existing Spec Touchpoints

- **Extends**: なし
- **Adjacent**: cluster-grain-typed-entity-facade（typed facade との語彙整合）、actor-kernel-public-surface-audit

## Constraints

`actor-core-typed` の facade は `actor-core-kernel` 上に構築する。default local receptionist の既存 behavior を壊さない。差し替え contract は最小にし、cluster runtime の詳細を typed core に持ち込まない。
