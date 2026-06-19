# Brief: actor-mailbox-resolution-contract

## Problem

mailbox runtime core は強いが、Pekko parity としては queue type / requirement ベースの mailbox selection contract が薄い。`RequiresMessageQueue[T]` / `ProducesMessageQueue[T]` 相当、`lookupByQueueType`、deploy / dispatcher / actor requirement / default の多段 precedence、BalancingDispatcher mailbox compatibility が未整理である。

## Current State

`MailboxRequirement` による capability 検証と `Props::with_mailbox_id` / `MailboxConfig` による単純な selection は存在する。主要 queue family も揃っている。一方で actor 宣言ベースの queue type 解決、mailbox id alias mapping、dispatcher 側 requirement との調停はない。

## Desired Outcome

queue semantics marker、RequiresMessageQueue / ProducesMessageQueue 相当、queue type から mailbox type を引く lookup contract、多段 mailbox selection precedence が kernel に定義される。BalancingDispatcher は multiple-consumer compatible mailbox の要求と検証を外部から見える mailbox contract として持つ。

## Approach

既存 `MailboxRequirement` と registry を捨てず、queue type resolution layer を追加する。selection precedence は HOCON 依存ではなく Rust の deploy / dispatcher / props / default source の順序として明文化する。

## Scope

- **In**: queue semantics marker、actor / mailbox type declaration、lookupByQueueType 相当、selection precedence、BalancingDispatcher compatibility contract、mailbox resolution tests
- **Out**: pushTimeOut 付き blocking bounded mailbox、JVM HOCON loading、Pekko alias chain の完全互換、testkit mailbox

## Boundary Candidates

- actor-declared requirement と mailbox-provided capability
- dispatcher config / deploy / props / default の selection source
- BalancingDispatcher shared queue と mailbox type compatibility

## Out of Boundary

- bounded mailbox blocking semantics
- mailbox run loop / scheduling gate の変更
- actor system registry split の構造変更そのもの

## Upstream / Downstream

- **Upstream**: actor-system-state-registry-split、既存 dispatch / mailbox implementation
- **Downstream**: actor-blocking-bounded-mailbox-compat、actor-kernel-public-surface-audit

## Existing Spec Touchpoints

- **Extends**: なし
- **Adjacent**: actor-mailbox-gap-analysis、actor-system-state-registry-split

## Constraints

`actor-core-kernel` の `no_std` 境界を維持する。async-first 方針を保ち、blocking semantics をこの spec に混ぜない。既存 mailbox id / config API は可能な限り bridge し、selection precedence の追加で利用者が意図しない queue へ silently fallback しないよう診断を明確にする。
