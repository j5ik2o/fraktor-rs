# actor contract coverage matrix

## 目的

Pekko 代表 Spec ごとに、fraktor-rs で public API または public state machine から検証済みの contract を `covered` として明示する。内部 helper の枝葉だけで coverage が増えた項目は `covered` にしない。

## 判定ルール

| 状態 | 意味 |
|------|------|
| `covered` | Rust の public API、integration test、または public state machine から代表 contract を観測できる。 |
| `deferred` | 実装対象だが、この change では専用 contract / E2E が不足している。 |
| `not covered` | fraktor-rs 側に対象機能が未実装、または actor-core 単体では再現できない。 |

## classic actor

| Pekko 代表 Spec | 状態 | 根拠 |
|-----------------|------|------|
| `ActorLifeCycleSpec.scala` | `covered` | `modules/actor-core/tests/system_lifecycle.rs`, `modules/actor-core/tests/supervisor.rs`, `modules/actor-core/tests/classic_user_flow_e2e.rs` で system 起動、child spawn、stop、termination を public API から検証する。 |
| `ActorMailboxSpec.scala` | `covered` | `modules/actor-core/tests/ping_pong.rs`, mailbox unit tests, `modules/actor-adaptor-std/tests/std_adaptor_boot_e2e.rs` で bounded mailbox / mailbox clock / dispatcher 接続を検証する。 |
| `DeathWatchSpec.scala` | `covered` | `modules/actor-core/tests/death_watch.rs`, `modules/actor-core/tests/classic_user_flow_e2e.rs` で watch / unwatch / terminated / dead letter を public API から検証する。 |
| `ReceiveTimeoutSpec.scala` | `covered` | `modules/actor-core/src/core/kernel/actor/actor_context/tests.rs` の receive-timeout contract が public state machine と marker API を検証する。 |
| `SchedulerSpec.scala` / `TimerSpec.scala` | `covered` | `modules/actor-core/tests/typed_scheduler.rs`, scheduler unit tests, `modules/actor-adaptor-std/tests/std_adaptor_boot_e2e.rs` で scheduler access と tick-driver 接続を検証する。 |
| `FSMActorSpec.scala` / `FSMTimingSpec.scala` / `FSMTransitionSpec.scala` | `covered` | `modules/actor-core/src/core/kernel/actor/fsm/tests.rs` が public FSM builder / state transition / timer contract を検証する。 |

## typed actor

| Pekko 代表 Spec | 状態 | 根拠 |
|-----------------|------|------|
| `BehaviorSpec.scala` | `covered` | `modules/actor-core/src/core/typed/tests.rs`, `modules/actor-core/src/core/typed/actor/actor_context/tests.rs` が `Behaviors.same` / behavior delegation / stopped 相当の public state machine を検証する。 |
| `ActorContextSpec.scala` | `covered` | `modules/actor-core/src/core/typed/actor/actor_context/tests.rs`, `modules/actor-core/tests/typed_user_flow_e2e.rs` で spawn / adapter / ask / pipeToSelf / stop を public context API から検証する。 |
| `WatchSpec.scala` | `covered` | `modules/actor-core/tests/typed_user_flow_e2e.rs` で typed watch と termination signal を public API から検証する。 |
| `SupervisionSpec.scala` | `covered` | `modules/actor-core/tests/supervisor.rs`, typed supervision strategy unit tests が restart / stop directive contract を検証する。 |
| `TimerSpec.scala` | `covered` | `modules/actor-core/tests/typed_scheduler.rs` と typed timer unit tests が scheduler / timer public contract を検証する。 |
| `AskSpec.scala` / `pipeToSelf` | `covered` | `modules/actor-core/src/core/typed/actor/actor_context/tests.rs`, `modules/actor-core/tests/typed_user_flow_e2e.rs` で ask / pipeToSelf の public API 経由 delivery を検証する。 |
| `EventStreamSpec.scala` | `covered` | `modules/actor-core/tests/event_stream.rs`, `modules/actor-core/tests/system_events.rs` が event stream / log / lifecycle event を public subscription API から検証する。 |

## typed testkit

| Pekko 代表 Spec | 状態 | 根拠 |
|-----------------|------|------|
| `ActorTestKitSpec.scala` | `deferred` | testkit 専用 API はこの change の actor-core / std adaptor scope から外す。専用 change で public testkit API を定義して検証する。 |
| `BehaviorTestKitSpec.scala` | `deferred` | behavior testkit の public harness は未整理。内部 behavior runner の unit coverage はあるが testkit contract としては扱わない。 |
| `TestProbeSpec.scala` | `deferred` | runtime probe helper は integration test 内に閉じており、Pekko testkit 互換 API としては未提供。 |

## Phase 10 で追加した網羅性ゲート

| ゲート | 根拠 |
|--------|------|
| classic E2E | `modules/actor-core/tests/classic_user_flow_e2e.rs` |
| typed E2E | `modules/actor-core/tests/typed_user_flow_e2e.rs` |
| std adaptor E2E | `modules/actor-adaptor-std/tests/std_adaptor_boot_e2e.rs` |
| sleep 依存排除 | `modules/actor-core/tests/death_watch.rs` と std dispatcher integration の待機を `yield_now` ベースに変更。 |
