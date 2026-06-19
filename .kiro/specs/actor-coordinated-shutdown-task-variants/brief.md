# Brief: actor-coordinated-shutdown-task-variants

## Problem

`CoordinatedShutdown` は基本 task registration と run contract を持つが、Pekko `addCancellableTask` / `addActorTerminationTask` 相当がない。shutdown 中に登録解除可能な task や actor termination を待つ task を表現できず、lifecycle parity に小さな穴が残っている。

## Current State

`actor-core-kernel/src/system/coordinated_shutdown.rs` は phase と task を持ち、`add_task` / `run` の基本機能を提供する。actor termination future や cancellable handle との統合は専用 API として露出していない。

## Desired Outcome

CoordinatedShutdown に cancellable task registration と actor termination task registration が追加される。actor termination task は既存 ActorRef / termination future / watch path と整合し、shutdown phase の順序と失敗処理を保つ。

## Approach

既存 task model を拡張し、cancellable registration は handle で task を解除できる contract にする。actor termination task は actor stop request と termination wait を合成する thin helper として実装し、shutdown engine 自体を大きく書き換えない。

## Scope

- **In**: cancellable task API、actor termination task API、phase ordering と cancellation semantics の tests、failure / timeout behavior の明文化
- **Out**: OS signal integration、cluster full shutdown command、new shutdown phase taxonomy、process exit handling

## Boundary Candidates

- task registration registry と run loop
- ActorRef termination waiting と shutdown phase
- cancellation handle と idempotency

## Out of Boundary

- SystemState registry split の構造変更そのもの
- remote / cluster coordinated shutdown integration
- metrics / tracing field contract

## Upstream / Downstream

- **Upstream**: actor-system-state-registry-split、既存 CoordinatedShutdown 実装
- **Downstream**: cluster lifecycle shutdown integration、showcase の coordinated shutdown example

## Existing Spec Touchpoints

- **Extends**: なし
- **Adjacent**: cluster-membership-event-surface（shutdown progress event とは別責務）

## Constraints

`actor-core-kernel` の `no_std` 境界を維持する。actor termination task は std blocking wait を持たず、既存 future / scheduler / actor system contract を使う。既存 `add_task` の後方挙動を変えない。
