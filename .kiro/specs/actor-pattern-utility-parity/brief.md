# Brief: actor-pattern-utility-parity

## Problem

actor gap Phase 1 には pattern utility の小さな欠けが残っている。`FutureTimeoutSupport.after` 相当の delayed future helper、ActorSelection に対する ask 合成、CircuitBreaker の状態遷移 listener と exponential backoff / random factor が未対応であり、Pekko pattern parity の利用体験が一段薄い。

## Current State

classic ask は `ActorRef` に対して timeout 付きで提供されている。`ActorSelection` の resolve path はあるが ask helper とは合成されていない。CircuitBreaker は Closed / Open / HalfOpen の状態を持つが、transition listener と reset timeout の指数バックオフ / jitter contract はない。

## Desired Outcome

kernel pattern module に scheduler-backed `after` helper、ActorSelection ask helper、CircuitBreaker transition listener と backoff / jitter 設定が追加される。既存 ask / retry / graceful_stop / circuit breaker API と整合し、std runtime へ直接依存しない。

## Approach

既存 `Scheduler` / `Clock` / `CircuitBreakerShared` を使い、delayed future と timeout behavior を core contract として表現する。ActorSelection ask は resolve + ActorRef ask の合成に留め、selection resolution 自体の仕様は変えない。

## Scope

- **In**: `after` helper、ActorSelection ask、CircuitBreaker on_open / on_close / on_half_open listener、exponential backoff / random factor 設定、deterministic tests
- **Out**: typed ask surface の再設計、scheduler runtime 実装の変更、retry policy の全面刷新

## Boundary Candidates

- scheduler-backed future helper と ask timeout
- ActorSelection resolution と ask composition
- CircuitBreaker state machine と listener notification

## Out of Boundary

- mailbox blocking semantics
- EventBus classification
- remote actor selection transport

## Upstream / Downstream

- **Upstream**: 既存 pattern / scheduler / actor selection 実装
- **Downstream**: typed facade utility の将来追加、showcase の pattern parity example

## Existing Spec Touchpoints

- **Extends**: なし
- **Adjacent**: actor-kernel-message-observability、actor-coordinated-shutdown-task-variants

## Constraints

`actor-core-kernel` に std sleep / Tokio time を持ち込まない。jitter は deterministic test が可能な injectable source か、既存 project pattern に合う設定型で扱う。listener callback は must-use / ignored return value lint に反しない形にする。
