# Brief: actor-kernel-message-observability

## Problem

actor gap Phase 1 には、FSM transition subscription、dead letter suppression、`PossiblyHarmful`、`WrappedMessage` という message observability 系の未対応が残っている。これらは remote / event stream / debugging の将来利用点になるが、kernel 側に marker と protocol がないため、外部から観測可能な契約として扱えない。

## Current State

FSM は `on_transition` closure による内部観測を持つが、外部 actor が transition を購読する message protocol はない。`DeadLetterReason` には suppressed 相当の variant があるが、ユーザーメッセージが抑制を宣言する marker と `SuppressedDeadLetter` 生成経路は未配線である。`PossiblyHarmful` と `WrappedMessage` 相当の marker / wrapper trait も存在しない。

## Desired Outcome

FSM transition subscription protocol が kernel に追加され、外部 actor が current state と transition を購読 / 解除できる。DeadLetterSuppression marker と SuppressedDeadLetter 生成経路が event stream に接続される。`PossiblyHarmful` と `WrappedMessage` は remote / event stream が将来参照できる最小 marker contract として定義される。

## Approach

Pekko の message names を参考にしつつ、Rust では enum / trait / marker 型として最小 surface を置く。ActorCell facet split 後の message dispatch / dead letter path に接続し、remote untrusted enforcement は本 spec では実装しない。

## Scope

- **In**: FSM CurrentState / SubscribeTransitionCallBack / UnsubscribeTransitionCallBack 相当、DeadLetterSuppression marker、SuppressedDeadLetter 生成経路、PossiblyHarmful marker、WrappedMessage trait、event stream tests
- **Out**: remote untrusted mode の遮断実装、cluster event integration、typed-only DSL sugar、testkit helpers

## Boundary Candidates

- FSM protocol と FSM runtime state
- dead letter generation と event stream publication
- marker traits と remote / serialization の将来利用点

## Out of Boundary

- 汎用 EventBus trait 族
- ActorSelection ask
- mailbox selection contract

## Upstream / Downstream

- **Upstream**: actor-cell-facet-structure
- **Downstream**: actor-eventbus-classification-contract、actor-kernel-public-surface-audit、remote untrusted message filtering の将来 spec

## Existing Spec Touchpoints

- **Extends**: なし
- **Adjacent**: remote gap work（PossiblyHarmful の利用先）、actor-eventbus-classification-contract

## Constraints

`actor-core-kernel` の `no_std` 境界で完結させる。marker は過剰な trait 階層にせず、実際に event stream / remote boundary から参照できる最小契約にする。既存 dead letter semantics を壊さない。
