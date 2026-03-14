# レビュー指摘事項

## Findings

- 高: `TypedActorContext::ask` / `ask_with_status` は Pekko の `ActorContext.ask` と違って、タイムアウトや失敗を actor 自身のメッセージに変換できません。現在の実装は単に message adapter を登録して request を送るだけで、`map_response` も成功値しか受け取れません。`modules/actor/src/core/typed/actor/actor_context.rs:402` `modules/actor/src/core/typed/actor/actor_context.rs:427` Pekko 側は `Try[Res] => T` を受け、`pipeToSelf(target.ask(...))` で timeout/failure も必ず self に戻します。`references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/internal/ActorContextImpl.scala:230` この差分のままだと、相手が応答しないケースで actor の状態遷移が止まります。

- 高: `TypedActorContext::ask` / `ask_with_status` は、同じ応答型に対する複数の未完了 ask を安全に扱えません。実装は `message_adapter` を使っていますが、adapter registry は payload の `TypeId` ごとに既存エントリを置換する仕様です。`modules/actor/src/core/typed/actor/actor_context.rs:413` `modules/actor/src/core/typed/actor/actor_context.rs:438` `modules/actor/src/core/typed/message_adapter/registry.rs:60` そのため、同一 actor が `u32` 返信の ask を2件並行で投げると後勝ちになり、先行 request の reply まで後続の `map_response` で解釈されます。

- 中: `ActorTags` 相当 API は `Props` に値を保持するだけで、runtime 側では実際に使われていません。`modules/actor/src/core/props/base.rs:18` `modules/actor/src/core/props/base.rs:70` `modules/actor/src/core/props/base.rs:164` Pekko では tags は logging marker として使われます。`references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Props.scala:235` 今回の実装だと公開 surface は増えていますが、利用者に見える挙動は増えていないので、機能追加としては未完です。

## 補足

`cargo test -p fraktor-actor-rs ask_sends_request_and_delivers_adapted_response --lib` は通過しました。ただし正常系 1 本だけで、上記の timeout/failure と複数同時 ask の問題はカバーしていません。

# actor モジュール ギャップ分析

Pekko互換仕様を実装する必要があります。
Phase 4: hard（アーキテクチャ変更を伴う）は対象外です。Phase 1から3を対応してください。
必要に応じて、Agent Teams, multi-agentsなどの機能を駆使してください。

## 前提

- 比較対象:
  - fraktor-rs 側: `modules/actor/src/` (core + std)
  - Pekko 側: `references/pekko/actor/src/` (classic) + `references/pekko/actor-typed/src/` (typed)
- fraktor-rs の `core/typed/` は Pekko `actor-typed` に、`core/actor/` は Pekko `actor` (classic) に対応する
- `core/` = no_std 層、`std/` = tokio/std アダプタ層

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（actor + actor-typed 代表 surface） | 約 85 |
| fraktor-rs 公開型数（core: 293, std: 27） | 320 |
| カバレッジ（代表 public surface） | 約 63/85 (74%) |
| ギャップ数（未対応 + 部分実装） | 22 |

生 count では fraktor-rs 側が多いが、これは core/std 分離、typed 同居、設定型・補助型の細分化による。
実質的な比較では、**基本 actor runtime + typed Behavior API はかなり揃っている一方、classic 互換 API、Reliable Delivery、CoordinatedShutdown、MDC 対応が不足**している。

## 層別カバレッジ

| 層 | Pekko 対応数 | fraktor-rs 実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 46 | 31 | 67% |
| core / typed ラッパー | 39 | 32 | 82% |
| std / アダプタ | 該当なし（Pekko は JVM 一体） | 27 | — |

## カテゴリ別ギャップ

### Actor Core (Classic) ✅ 実装済み 5/9 (56%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ReceiveTimeout` (classic) | `Actor.scala:154` | 部分実装 | core/actor | medium | typed 側の receive-timeout はあるが、classic untyped の公開 API はない |
| `become` / `unbecome` | `Actor.scala`, `AbstractActor.scala` | 未対応 | core/actor | hard | fraktor untyped actor は behavior stack を公開していない |
| `ActorRef.forward` | `ActorRef.scala:154` | 未対応 | core/actor | easy | `tell` はあるが classic `forward` 相当の公開メソッドは未提供 |
| `ActorRef.noSender` | `ActorRef.scala:35` | 未対応 | core/actor | trivial | sender 省略は可能だが、同名 sentinel API はない |

**実装済み（テーブル省略）**: `Actor` trait, `ActorCell`, `ActorContext`, `ActorRef`, `PoisonPill`（`SystemMessage::PoisonPill`）, `Kill`（`SystemMessage::Kill`）, `Identify`/`ActorIdentity`, `Status`

### ActorPath / ActorSelection ✅ 実装済み 5/7 (71%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorSelection` handle API | `ActorSelection.scala:35` | 部分実装 | core/actor | medium | fraktor は `ActorSelectionResolver` まで。Pekko の selection handle API は未提供 |
| `Address` (フル公開 API) | `Address.scala:120` | 部分実装 | core/system | easy | `RemotingConfig` と `RemoteAuthorityRegistry` で代替しているが同名 API はない |

**実装済み**: `ActorPath`, `RootActorPath` 相当, `ActorPathParser`, `Uid`, `Segment`, `ActorPathScheme`, `GuardianKind`

### ActorSystem / Bootstrap / Extension ✅ 実装済み 6/10 (60%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `CoordinatedShutdown` | `CoordinatedShutdown.scala:41` | 未対応 | core/system | hard | terminate はあるが phase 付き coordinated shutdown はない |
| `ActorSystemSetup` / `BootstrapSetup` | `ActorSystem.scala:41` | 未対応 | core/system | medium | fraktor は `ActorSystemConfig` ベースで、setup 合成 DSL はない |
| `DynamicAccess` | `DynamicAccess.scala` | 未対応 | — | n/a | JVM reflection 前提。Rust では直接移植の価値なし |
| `ClassicActorSystemProvider` | `ClassicActorSystemProvider.scala` | 未対応 | — | n/a | classic/typed ブリッジ。Rust では不要 |

**実装済み**: `ActorSystem`, `ExtendedActorSystem`, `ActorSystemConfig`, `Extension`, `ExtensionId`, `ExtensionInstaller`

### Typed Behavior DSL ✅ 実装済み 15/19 (79%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Behaviors.receivePartial` | `Behaviors.scala:161` | 未対応 | core/typed | easy | Rust ではパターンマッチで `receive_message` 内対応可能だが、便利メソッドとしてはない |
| `Behaviors.receiveMessagePartial` | `Behaviors.scala:169` | 未対応 | core/typed | easy | 同上 |
| `Behaviors.withMdc` | `Behaviors.scala:285` | 未対応 | std/typed | medium | tracing span で代替可能だが公開 API はない |
| `AbstractBehavior` (OOP スタイル) | `AbstractBehavior.scala:46` | 未対応 | core/typed | medium | fraktor は FP スタイルのみ。OOP ベース actor は未提供 |

**実装済み**: `Behavior<M>`, `Behaviors::setup`, `same`, `stopped`, `ignore`, `unhandled`, `empty`, `receive_message`, `receive_and_reply`, `receive_signal`, `with_stash`, `with_timers`, `intercept`, `intercept_behavior`, `intercept_signal`, `monitor`, `supervise`

### Typed Signals ✅ 実装済み 6/6 (100%)

**実装済み**: `BehaviorSignal` enum に以下すべて実装
- `Started` (≈ Pekko の setup 初期化)
- `Stopped` (≈ `PostStop`)
- `PreRestart`
- `Terminated(Pid)`
- `ChildFailed { pid, error }`
- `MessageAdaptionFailure`

### Typed ActorContext ✅ 実装済み 14/17 (82%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `scheduleOnce` (on context) | `ActorContext.scala:232` | 未対応 | core/typed | easy | `TimerScheduler` 経由で代替可能だが context 直接メソッドはない |
| `ask` / `askWithStatus` (on context) | `ActorContext.scala:319-328` | 未対応 | core/typed | medium | `TypedAskFuture` は存在するが context 上の便利メソッドは未実装 |
| `executionContext` (implicit) | `ActorContext.scala:241` | 未対応 | — | n/a | Rust では不要（async ランタイムが異なる） |

**実装済み**: `self_ref`, `system`, `spawn_child`, `spawn_child_watched`, `watch`, `watch_with`, `unwatch`, `stop_self`, `stop_child`, `stop_actor_by_ref`, `children`, `child`, `stash`, `unstash`, `unstash_all`, `delegate`, `message_adapter`, `spawn_message_adapter`, `pipe_to_self`, `set_receive_timeout`, `cancel_receive_timeout`

### Typed Props / Selectors ✅ 実装済み 4/5 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorTags` | `Props.scala:241` | 未対応 | core/typed | easy | メタデータタグ。fraktor には未実装 |

**実装済み**: `TypedProps<M>`, `DispatcherSelector`, `MailboxSelector`, `LogOptions`

### Supervision ✅ 実装済み 6/6 (100%)

**実装済み**:
- `SupervisorStrategy` (resume, restart, stop, escalate)
- `BackoffSupervisorStrategy` (restart with backoff)
- `SupervisorDirective`
- `SupervisorStrategyConfig` (OneForOne, AllForOne)
- `RestartStatistics`
- `Supervise<M>` (typed DSL)

### Scheduler / Timers ✅ 実装済み 7/8 (88%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| classic `Timers` mixin | `Timers.scala` | 未対応 | core/actor | medium | typed TimerScheduler で代替しているが classic mixin surface はない |

**実装済み**: `Scheduler`, `SchedulerShared`, `TimerScheduler<M>`, `TimerKey`, `Cancellable` 相当, `Behaviors::with_timers`, `start_timer_with_fixed_delay`, `start_timer_at_fixed_rate`, `start_single_timer`, `cancel`, `cancel_all`, `is_timer_active`

### Stash ✅ 実装済み 1/2 (50%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| classic `Stash` / `UnboundedStash` trait | `Stash.scala:71-82` | 未対応 | core/actor | medium | fraktor は typed `StashBuffer<M>` のみ。classic trait ベースではない |

**実装済み**: `StashBuffer<M>` (typed) — `stash`, `unstash`, `unstash_all`, `clear`, `head`, `contains`, `exists`, `foreach`, `is_empty`, `is_full`, `capacity`, `len`

### Receptionist ✅ 実装済み 4/4 (100%)

**実装済み**: `Receptionist`, `ServiceKey<M>`, `ReceptionistCommand`, `Listing`

### Routers ✅ 実装済み 3/4 (75%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Resizer` (動的 routee resize) | `routing/Resizer.scala:40` | 未対応 | core/typed | hard | 動的 routee resize は未実装 |

**実装済み**: `Routers`, `GroupRouterBuilder<M>` (random, round-robin, consistent-hash routing), `PoolRouterBuilder<M>` (broadcast, round-robin, random, consistent-hash, smallest-mailbox)

### PubSub / Topic ✅ 実装済み 3/3 (100%)

**実装済み**: `Topic`, `TopicCommand<M>`, `TopicStats`

### SpawnProtocol ✅ 実装済み 1/1 (100%)

**実装済み**: `SpawnProtocol`

### Ask Pattern ✅ 実装済み 2/3 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `AskPattern` (extension method) | `AskPattern.scala:38` | 未対応 | core/typed | medium | `TypedAskFuture`/`TypedAskResponse` は存在するが、Pekko の `?` 演算子相当の便利 API はない |

**実装済み**: `TypedAskFuture<R>`, `TypedAskResponse<R>`, `TypedAskError`, `StatusReply<T>`

### Dispatchers / Mailbox ✅ 実装済み 8/8 (100%)

**実装済み**: `Dispatchers`, `DispatcherConfig`, `DispatchExecutor`, `Mailbox`, `MailboxType`, `MessageQueue`, `BoundedMailboxType`, `UnboundedMailboxType`, `BoundedMessageQueue`, `UnboundedMessageQueue`, `OverflowStrategy`, `MailboxCapacity`

### Event Stream / Logging ✅ 実装済み 4/6 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `LoggingAdapter` / `DiagnosticLoggingAdapter` | `event/Logging.scala` | 未対応 | std | medium | fraktor は event stream + subscriber ベース。Pekko の adapter 階層は未対応 |
| `DeadLetterListener` actor | `event/DeadLetterListener.scala` | 未対応 | core | easy | dead letter store はあるが listener actor の classic surface はない |

**実装済み**: `EventStream`, `EventStreamSubscriber`, `LogEvent`, `LogLevel`, `LoggerSubscriber`, `TracingLoggerSubscriber`, `DeadLetter`

### Serialization ✅ 実装済み 6/6 (100%)

**実装済み**: `Serializer`, `SerializerId`, `SerializationRegistry`, `SerializationSetup`, `SerializedMessage`, `ConfigAdapter`
（Rust の serde/bincode ベース設計とは責務境界が異なるが、必要な API は揃っている）

### Extensions ✅ 実装済み 3/3 (100%)

**実装済み**: `Extension` (trait), `ExtensionId`, `ExtensionSetup<I>`

### Reliable Delivery ❌ 未対応 0/4 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ProducerController` | `delivery/ProducerController.scala` | 未対応 | core/typed | hard | at-least-once delivery パターン |
| `ConsumerController` | `delivery/ConsumerController.scala` | 未対応 | core/typed | hard | flow-control 付きメッセージ配信 |
| `WorkPullingProducerController` | `delivery/WorkPullingProducerController.scala` | 未対応 | core/typed | hard | pull ベースの work distribution |
| `DurableProducerQueue` | `delivery/DurableProducerQueue.scala` | 未対応 | core/typed | hard | 永続化キュー（persistence 連携） |

### 対象外（n/a）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `IO/Tcp/Udp/Dns` 系 | `io/Tcp.scala` 等 | 対象外 | n/a | remote/network サブシステムの領域 |
| Java DSL (`javadsl`) | `javadsl/Behaviors.scala` 等 | 対象外 | n/a | JVM 言語バインディング。Rust では不要 |
| `DynamicAccess` / `ReflectiveDynamicAccess` | `DynamicAccess.scala` | 対象外 | n/a | JVM reflection 前提 |
| `ClassicActorSystemProvider` / `ClassicActorContextProvider` | | 対象外 | n/a | classic/typed ブリッジ。Rust では不要 |
| `ActorRefResolverSetup` / `ReceptionistSetup` | | 対象外 | n/a | typed→classic ブリッジ setup |
| `AbstractFSM` (classic) | `AbstractFSM.scala` | 対象外 | n/a | classic FSM は deprecated。typed `FsmBuilder` で代替 |
| `Adapter` (classic→typed ブリッジ) | `adapter/` | 対象外 | n/a | Rust では不要 |
| `BehaviorInterceptor.PreStartTarget` / `ReceiveTarget` / `SignalTarget` | | 対象外 | n/a | Rust では `FnMut` で代替済み |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- `ActorRef.noSender` 相当の sentinel API — core/actor
- `DeadLetterListener` 相当の公開 listener surface — core

### Phase 2: easy（単純な新規実装）
- `ActorRef.forward` の公開メソッド追加 — core/actor
- `Behaviors.receivePartial` / `receiveMessagePartial` 便利メソッド — core/typed
- `ActorTags` 相当のメタデータ — core/typed
- `Address` のフル公開 API — core/system
- `ActorContext.scheduleOnce` 便利メソッド — core/typed

### Phase 3: medium（中程度の実装工数）
- `ActorSelection` handle API の拡充 — core/actor
- `AskPattern` 便利 API（context 上 `ask`/`askWithStatus`）— core/typed
- `Behaviors.withMdc` / MDC 対応 — std/typed
- `AbstractBehavior` 相当の OOP スタイル actor base — core/typed
- `ActorSystemSetup` / `BootstrapSetup` 合成 DSL — core/system
- classic `ReceiveTimeout` 公開 API — core/actor
- classic `Stash` / `UnboundedStash` trait — core/actor
- classic `Timers` mixin — core/actor

### Phase 4: hard（アーキテクチャ変更を伴う）
- `CoordinatedShutdown` phase model — core/system（core 層の変更が必要、std 層にも波及）
- `Reliable Delivery`（`ProducerController` / `ConsumerController` / `WorkPullingProducerController`）— core/typed（persistence 連携が必須）
- `Resizer`（動的 routee resize）— core/typed
- `become` / `unbecome` classic behavior stack — core/actor

### 対象外（n/a）
- `IO/Tcp/Udp/Dns` — network サブシステム
- `javadsl` — JVM 言語バインディング
- `DynamicAccess` / `ReflectiveDynamicAccess` — JVM reflection
- classic/typed ブリッジ API — Rust では不要
- `AbstractFSM` (classic) — deprecated。typed `FsmBuilder` で代替

## まとめ

- 全体カバレッジは約 **74%**。主要機能は概ねカバー済みで、**typed Behavior DSL、Supervision、Scheduler/Timers、Receptionist、Routers、PubSub/Topic、Serialization がほぼ完備**している。
- **即座に価値を提供できる未実装機能**（Phase 1〜2）: `ActorRef.forward`, `receivePartial` 系便利メソッド, `ActorTags`, `scheduleOnce` on context。いずれも既存基盤の薄いラッパーで実装可能。
- **実用上の主要ギャップ**（Phase 3〜4）: `CoordinatedShutdown`（graceful shutdown のフレームワーク）、`Reliable Delivery`（at-least-once + flow control、persistence 連携必須）、`AskPattern` 便利 API、`AbstractBehavior`（OOP スタイル actor）。
- **YAGNI 観点での省略推奨**: classic 互換 API（`become/unbecome`、classic `Stash`、classic `Timers`）は typed API で代替済みのため、fraktor-rs が typed-first を貫くなら意図的に非実装とするのが妥当。`Reliable Delivery` は persistence モジュールの成熟度に依存するため、時期尚早の可能性がある。
