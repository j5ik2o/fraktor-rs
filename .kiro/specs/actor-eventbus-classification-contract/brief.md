# Brief: actor-eventbus-classification-contract

## Problem

fraktor-rs の EventStream は実用的だが、Pekko の汎用 EventBus trait 族（Lookup / Subchannel / Scanning / ManagedActor classification）に相当する拡張契約がない。ユーザー定義の event bus や classification strategy を構築できず、event / logging カテゴリの主要 medium gap として残っている。

## Current State

`actor-core-kernel/src/event/stream.rs` は fixed classifier と subscriber model を持つ。dead letter / unhandled message / logging event は流せるが、classification trait を外部実装して独自 EventBus を組み立てる surface はない。

## Desired Outcome

EventBus、ActorEventBus、LookupClassification、SubchannelClassification、ScanningClassification、ManagedActorClassification 相当の trait / helper が kernel に定義される。既存 EventStream は互換性を保ちつつ、どの分類 strategy を採用しているかが明確になる。

## Approach

Pekko の trait 階層を Rust の trait + associated type / generic helper に変換する。既存 EventStream を一気に置き換えず、まず汎用 classification contract と既存 EventStream への bridge を定義する。

## Scope

- **In**: EventBus trait 族、classification strategy trait、actor subscriber 管理 contract、既存 EventStream との bridge / tests
- **Out**: logging backend の置換、cluster pubsub mediator、typed eventstream の再設計、すべての event publication path の一括 rewrite

## Boundary Candidates

- generic EventBus contract と concrete EventStream
- classifier strategy と subscriber registry
- actor subscriber lifecycle と dead letter / unhandled message publication

## Out of Boundary

- DeadLetterSuppression marker の導入そのもの
- mailbox selection / dispatcher contract
- std tracing subscriber

## Upstream / Downstream

- **Upstream**: actor-system-state-registry-split、actor-kernel-message-observability
- **Downstream**: actor-kernel-public-surface-audit、将来の custom event bus / logging extension

## Existing Spec Touchpoints

- **Extends**: なし
- **Adjacent**: actor-kernel-message-observability、cluster-pubsub-mediator-protocol

## Constraints

`actor-core-kernel` の `no_std` + alloc で実装可能な trait にする。Pekko の継承構造をそのまま移植せず、Rust の trait composition に寄せる。既存 EventStream の public behavior を壊さない。
