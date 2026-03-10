# actor モジュール ギャップ分析

> 分析日: 2026-03-10  
> 対象:
> - fraktor-rs: `modules/actor/src/`
> - Pekko: `references/pekko/actor/src/main/scala/org/apache/pekko/{actor,pattern}` と `references/pekko/actor/src/main/java/org/apache/pekko/{actor,pattern}`

## サマリー

| 指標 | 値 |
|---|---:|
| Pekko 公開型数 | 194 |
| fraktor-rs 公開型数 | 354 |
| 同名型カバレッジ | 14/194 (7.2%) |
| 主要ギャップ項目 | 11 |

注記:
- 同名型カバレッジは「型名一致」のみを数えるため、別名実装は過小評価になる。
- `fraktor-rs` は 1機能を複数の小型公開型へ分割する設計のため、公開型数は Pekko より多くなる。

## カテゴリ別ギャップ

### 型・トレイト

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `Actor` / `ActorRef` / `ActorSystem` / `Props` | `Actor.scala`, `ActorRef.scala`, `ActorSystem.scala`, `Props.scala` | 実装済み (`Actor`, `ActorRef`, `ActorSystem`, `Props`) | - | 基本サーフェスは対応 |
| `SupervisorStrategy` (`OneForOne`/`AllForOne`) | `FaultHandling.scala` | 実装済み (`SupervisorStrategyKind`) | - | 方針種別は対応 |
| `ActorSelection` | `ActorSelection.scala:39` | 部分対応 (`ActorSelectionResolver`) | medium | Selection オブジェクトとしての送信/ask 面は未提供 |
| `Stash` / `UnboundedStash` / `UnrestrictedStash` | `Stash.scala:71` | 部分対応 (`typed::StashBuffer`) | medium | クラシック API 互換は未提供 |
| `FSM` | `FSM.scala:430` | 部分対応 (`typed::FsmBuilder`) | medium | クラシック FSM trait 互換は未提供 |
| `Timers` (classic) | `Timers.scala:31` | 部分対応 (`Behaviors::with_timers`) | medium | typed 寄りで classic `Timers` trait は未提供 |
| `CoordinatedShutdown` | `CoordinatedShutdown.scala:41` | 未対応 | hard | 拡張ポイント/フェーズ実行モデルが未実装 |
| `Identify` / `ActorIdentity` | `Actor.scala:81`, `Actor.scala:91` | 未対応 | easy | 運用ユーティリティとしては追加容易 |
| `ReceiveTimeout` (classic) | `Actor.scala:154` | 部分対応 (`TypedActorContext::set_receive_timeout`) | medium | classic 側 API がない |

### パターン API

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `ask(actorRef, timeout)` | `Patterns.scala:78`, `AskSupport.scala:93` | 部分対応 (`ActorRef::ask`) | easy | timeout 引数を公開 API で指定できない |
| `ask(actorSelection, timeout)` | `Patterns.scala:237`, `AskSupport.scala:159` | 未対応 | medium | `ActorSelection` 実体不足に依存 |
| `askWithStatus` | `AskSupport.scala:103` | 実装済み (`TypedActorRef::ask_with_status`) | - | typed 面で対応 |
| `pipeTo` / `pipeToSelection` | `PipeToSupport.scala:31`, `PipeToSupport.scala:37` | 部分対応 (`pipe_to_self`) | medium | 他 actor/selection への pipe API が未提供 |
| `gracefulStop` | `GracefulStopSupport.scala:59`, `Patterns.scala:387` | 未対応 | medium | `terminate` はあるが対象 actor 単位 graceful stop がない |
| `BackoffSupervisor` / `BackoffOpts` | `BackoffSupervisor.scala:22`, `BackoffOptions.scala:27` | 部分対応 (`BackoffSupervisorStrategy`) | easy | オプション DSL 互換は未提供 |
| `RetrySupport` | `RetrySupport.scala:30` | 未対応 | easy | 補助ユーティリティとして切り出し可能 |
| `CircuitBreaker` / `CircuitBreakersRegistry` | `CircuitBreaker.scala:133`, `CircuitBreakersRegistry.scala:35` | 未対応 | hard | 実装追加で actor 境界を超える責務増が大きい |
| `AskTimeoutException` | `AskSupport.scala:38` | 部分対応 (`AskError::Timeout`) | easy | 例外型互換ではなく enum エラー表現 |

### ライフサイクル/制御メッセージ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|---|---|---|---|---|
| `PoisonPill` / `Kill` | `Actor.scala:52`, `Actor.scala:67` | 実装済み (`SystemMessage::PoisonPill` / `Kill`, `ActorRef::poison_pill`, `ActorRef::kill`) | - | 互換挙動を提供 |

## 実装優先度の提案

### Phase 1: easy（最小追加で効果が高い）
- `ActorRef::ask` に timeout 指定を追加（既存 `AskError::Timeout` を活用）
- `graceful_stop` ヘルパを追加（対象 actor 停止 + 完了待機）
- `RetrySupport` 相当を薄いユーティリティとして追加

### Phase 2: medium（互換面を広げる）
- `ActorSelection` を resolver から一段上げ、`tell`/`ask` を持つ実体 API を追加
- `pipe_to` / `pipe_to_selection` 相当を追加（`pipe_to_self` 依存で実装）
- classic 側 `ReceiveTimeout` API の橋渡し
- classic `Stash` 互換レイヤー（typed `StashBuffer` を下位に利用）

### Phase 3: hard（基盤設計を伴う）
- `CoordinatedShutdown` 相当（フェーズ/依存順序/期限管理）
- `CircuitBreaker` + registry

### 対象外（n/a）
- JVM/Java DSL 固有サーフェス（`AbstractActor` 系 Java API 互換の完全再現）
- Scala implicit 前提の記法互換（`?` 演算子等）

## 根拠（主要参照）

- Pekko:
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorSelection.scala:39`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Stash.scala:71`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FSM.scala:430`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/CoordinatedShutdown.scala:41`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/pattern/Patterns.scala:78`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/pattern/Patterns.scala:387`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/pattern/PipeToSupport.scala:31`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/pattern/CircuitBreaker.scala:133`

- fraktor-rs:
  - `modules/actor/src/core/actor/actor_ref/base.rs:27`
  - `modules/actor/src/core/actor/actor_ref/base.rs:111`
  - `modules/actor/src/core/actor/actor_ref/base.rs:131`
  - `modules/actor/src/core/actor/actor_selection/resolver.rs:13`
  - `modules/actor/src/core/typed/stash_buffer.rs:11`
  - `modules/actor/src/core/typed/fsm_builder.rs:18`
  - `modules/actor/src/core/typed/behaviors.rs:192`
  - `modules/actor/src/core/typed/actor/actor_context.rs:284`
  - `modules/actor/src/core/typed/actor/actor_ref.rs:97`
  - `modules/actor/src/core/supervision/backoff_supervisor_strategy.rs:18`
  - `modules/actor/src/std/system/base.rs:36`
