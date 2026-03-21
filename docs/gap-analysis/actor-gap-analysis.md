# actor モジュール ギャップ分析

更新日: 2026-03-22

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（意味のある型単位） | 約 120（classic: ~70, typed: ~50） |
| fraktor-rs 公開型数 | 約 440（core: 418, std: 22） |
| カバレッジ（型単位） | 約 119/120 (99%) |
| ギャップ数 | 1（core/typed + persistence: 1） |

※ Java API 重複（AbstractBehavior, ReceiveBuilder, javadsl.* 等）、private[pekko] 内部型、JVM 固有機能（Deploy, IO, DynamicAccess 等）は Pekko 側計数から除外。
※ fraktor-rs 側は参照実装以上に細粒度で型分割（type-per-file ルール）しているため型数が多いが、概念単位でのカバレッジを評価する。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | ~55 | 330 | 100% |
| core / typed ラッパー | ~50 | 88 | 98% |
| std / アダプタ | 1 | 22 | 100% |

※ fraktor-rs 側の型数は `rg '^\\s*pub ... (struct|trait|enum|type)'` による機械抽出ベース。`type-per-file` 方針のため、Pekko より細かく分割されている。

## カテゴリ別ギャップ

### コア型（ActorRef, ActorSystem, Props, ActorContext） ✅ 実装済み 12/12 (100%)

全主要型が実装済み。ギャップなし。
ActorRef, ActorSystem, Props, ActorContext, ActorCell, ActorRefProvider, ActorRefFactory, LocalActorRefProvider 相当がすべて存在。

### Address / ActorPath ✅ 実装済み 5/5 (100%)

全主要型が実装済み。Address, ActorPath, RootActorPath, ChildActorPath, ActorPathParser が存在。
前回分析で未対応だった `RootActorPath`, `ChildActorPath` が実装された。

### メッセージ型・シグナル ✅ 実装済み 14/14 (100%)

全主要型が実装済み。
- Terminated（BehaviorSignal::Terminated）, ReceiveTimeout, UnhandledMessage（UnhandledMessageEvent）
- PreRestart（BehaviorSignal::PreRestart）, PostStop（BehaviorSignal::Stopped + TypedActor::post_stop）
- ChildFailed（BehaviorSignal::ChildFailed）, MessageAdaptionFailure（BehaviorSignal::MessageAdaptionFailure）
- Signal（BehaviorSignal）, PoisonPill（SystemMessage::PoisonPill）, Kill（SystemMessage::Kill）
- DeathPactException（`core/typed/death_pact_exception.rs`）
- Status（StatusReply）, Identify / ActorIdentity

### Dead Letters ✅ 実装済み 3/3 (100%)

全主要型が実装済み。DeadLetter, SuppressedDeadLetter（DeadLetterReason::SuppressedDeadLetter）, Dropped（DeadLetterReason::Dropped）。

### Behaviors ファクトリ ✅ 実装済み 16/16 (100%)

全主要 API が実装済み。
- core 層: setup, receive_message, receive_and_reply, receive_message_partial, receive_partial, receive_signal, same, stopped, unhandled, ignore, with_stash, with_timers, intercept, intercept_behavior, intercept_signal, transform_messages, monitor
- std 層: log_messages, log_messages_with_opts, with_mdc, with_static_mdc
- Behavior: narrow, receive_signal, with_supervisor_strategy, transform_messages

前回分析で未対応だった `Behavior.narrow`, `Behaviors.transformMessages` が実装された。

※ `receivePartial` / `receiveMessagePartial` は Scala の PartialFunction 固有。fraktor-rs では `receive_message_partial` / `receive_partial` として Rust 流の実装あり。

### Supervision ✅ 実装済み 5/5 (100%)

全主要型が実装済み。SupervisorStrategy（resume/restart/stop）, BackoffSupervisorStrategy, Supervise ビルダー, OneForOneStrategy（SupervisorStrategyKind::OneForOne）, AllForOneStrategy（SupervisorStrategyKind::AllForOne）, SupervisorDirective, RestartStatistics。

### BehaviorInterceptor ✅ 実装済み 2/2 (100%)

BehaviorInterceptor, BehaviorSignalInterceptor が実装済み。

### ActorContext (typed) ✅ 実装済み 14/14 (100%)

全主要 API が実装済み。
- self_ref, system, tags, spawn_child, spawn_child_watched, stop_self, stop_child, stop_actor_by_ref
- watch, watch_with, unwatch, children, child
- delegate, forward, schedule_once, message_adapter, spawn_message_adapter
- pipe_to_self, set_receive_timeout, cancel_receive_timeout
- ask, ask_with_status
- stash, stash_with_limit, unstash, unstash_all

前回分析で未対応だった `ActorContext.scheduleOnce` が実装された。

### Receptionist / ServiceKey ✅ 実装済み 7/7 (100%)

全主要型・コマンドが実装済み。Receptionist, ServiceKey, Listing, ReceptionistCommand（Register, Deregister, Subscribe, Unsubscribe）。

### Router ✅ 実装済み 11/11 (100%)

全主要ルーティング戦略が実装済み。
- Routers: pool, group, scatter_gather_first_completed_pool, balancing_pool, tail_chopping_pool
- PoolRouterBuilder: with_broadcast, with_round_robin, with_random, with_consistent_hash, with_broadcast_predicate, with_smallest_mailbox, with_resizer
- GroupRouterBuilder: with_random_routing, with_round_robin_routing, with_consistent_hash_routing
- ScatterGatherFirstCompletedRouterBuilder, TailChoppingRouterBuilder, BalancingPoolRouterBuilder
- Resizer trait, DefaultResizer

前回分析で未対応だった `ScatterGatherFirstCompleted`, `TailChopping`, `BalancingPool`, `Resizer` が全て実装された。

### Timer / Stash ✅ 実装済み 4/4 (100%)

TimerScheduler（startTimerWithFixedDelay, startTimerAtFixedRate, startSingleTimer, isTimerActive, cancel, cancelAll）, TimerKey, StashBuffer が実装済み。

### SpawnProtocol ✅ 実装済み 1/1 (100%)

SpawnProtocol が `core/typed/spawn_protocol.rs` に実装済み。

### LogOptions ✅ 実装済み 1/1 (100%)

LogOptions が `std/typed/log_options.rs` に実装済み（withEnabled, withLevel, withLoggerName）。

### ActorRefResolver ✅ 実装済み 1/1 (100%)

ActorRefResolver が `core/typed/actor_ref_resolver.rs` に実装済み。

### Ask パターン ✅ 実装済み 4/4 (100%)

ask_with_timeout, ask on context, ask_with_status, pipe_to_self, StatusReply が実装済み。TypedAskFuture, TypedAskResponse, TypedAskError も存在。

### EventStream ✅ 実装済み 3/3 (100%)

EventStream（subscribe, unsubscribe, publish）が実装済み。EventStreamShared, EventStreamSubscription も存在。typed EventStream コマンド（Publish, Subscribe, Unsubscribe）も対応。

### Topic / PubSub ✅ 実装済み 5/5 (100%)

Topic, TopicCommand（Publish, Subscribe, Unsubscribe, GetTopicStats）, TopicStats が実装済み。

### Extension ✅ 実装済み 4/4 (100%)

Extension, ExtensionId, ExtensionInstaller, ExtensionSetup が実装済み。

### FSM ✅ 実装済み 1/1 (100%)

FsmBuilder が typed 層に存在。Pekko の classic FSM は複雑な Scala DSL だが、fraktor-rs は typed 層で簡潔に実装。

### Dispatch / Mailbox ✅ 実装済み 12/12 (100%)

全主要メールボックス型が実装済み。
- MailboxType, MessageQueue, DequeMessageQueue
- UnboundedMailboxType, UnboundedMessageQueue
- BoundedMailboxType, BoundedMessageQueue
- UnboundedPriorityMailboxType, UnboundedPriorityMessageQueue
- BoundedPriorityMailboxType, BoundedPriorityMessageQueue
- UnboundedStablePriorityMailboxType, UnboundedStablePriorityMessageQueue
- BoundedStablePriorityMailboxType, BoundedStablePriorityMessageQueue
- UnboundedDequeMailboxType, UnboundedDequeMessageQueue
- UnboundedControlAwareMailboxType, UnboundedControlAwareMessageQueue
- Dispatchers, DispatcherConfig, DispatchExecutor, PinnedDispatcher (std)
- MailboxCapacity, MailboxOverflowStrategy, MailboxInstrumentation, MessagePriorityGenerator
- BackpressurePublisher, MailboxPolicy, ScheduleHints

前回分析で未対応だった全メールボックスバリエーション（PriorityMailbox群、DequeMailbox、ControlAwareMailbox、PinnedDispatcher）が実装された。

### Serialization ✅ 実装済み 6/6 (100%)

全主要型が実装済み。
- Serializer, SerializerWithStringManifest, SerializationRegistry, SerializationExtension
- ByteBufferSerializer（`core/serialization/byte_buffer_serializer.rs`）
- AsyncSerializer（`core/serialization/async_serializer.rs`）
- 組み込みシリアライザ（Bool, I32, Null, String, Bytes）

前回分析で未対応だった `ByteBufferSerializer`, `AsyncSerializer` が実装された。

### Pattern（ユーティリティ） ✅ 実装済み 5/5 (100%)

- ask_with_timeout, graceful_stop, graceful_stop_with_message, retry
- CircuitBreaker（std）: CircuitBreaker, CircuitBreakerShared, CircuitBreakerState, CircuitBreakerCallError, CircuitBreakerOpenError
- StatusReply, pipe_to_self（typed context 上）

前回分析で未対応だった `CircuitBreaker` が実装された。

### Util（ユーティリティ型） ✅ 実装済み 3/3 (100%)

全主要型が実装済み。
- ByteString（`core/messaging/byte_string.rs`）
- MessageBuffer（`core/messaging/message_buffer.rs`）
- MessageBufferMap（`core/messaging/message_buffer_map.rs`）

前回分析で未対応だった `ByteString` が実装された。

### CoordinatedShutdown ✅ 実装済み 1/1 (100%)

`CoordinatedShutdown` が std 層に実装済み。
- CoordinatedShutdown（`std/system/coordinated_shutdown.rs`）
- CoordinatedShutdownPhase（`std/system/coordinated_shutdown_phase.rs`）
- CoordinatedShutdownReason（`std/system/coordinated_shutdown_reason.rs`）
- CoordinatedShutdownId（`std/system/coordinated_shutdown_id.rs`）
- CoordinatedShutdownInstaller（`std/system/coordinated_shutdown_installer.rs`）
- CoordinatedShutdownError（`std/system/coordinated_shutdown_error.rs`）

Pekko のフェーズ付きシャットダウン API に対して、fraktor-rs では std アダプタとして一式が揃った。

### Reliable Delivery（信頼性メッセージング） ✅ 実装済み 3/4 (75%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `DurableProducerQueue` | `actor-typed/.../delivery/DurableProducerQueue.scala:33` | 未対応 | core/typed + persistence | hard | 永続化キュー。persistence モジュールと typed delivery の接続が未整備 |

実装済み:
- `ProducerController`（`core/typed/delivery/producer_controller.rs`）
- `ConsumerController`（`core/typed/delivery/consumer_controller.rs`）
- `WorkPullingProducerController`（`core/typed/delivery/work_pulling_producer_controller.rs`）
- `SequencedMessage`, `SeqNr`, `ConsumerControllerDelivery`, `ProducerControllerRequestNext` など周辺プロトコル一式

### その他の typed API ✅ 実装済み 2/2 (100%)

ActorTags は TypedActorContext.tags() として実装済み。
Behavior.transformMessages は `core/typed/behaviors.rs` に実装済み。

### スタブ / 未完成実装　✅ 実装済み 0/0 (100%)

`modules/actor/src` を対象に `todo!()`, `unimplemented!()`, `panic!(\"not implemented\")`, `TODO` を検索した範囲では、公開 API 直下のスタブ痕跡は見つからなかった。

### 対象外（n/a）

| Pekko API | 理由 |
|-----------|------|
| Java API 重複（`javadsl.*`, `AbstractBehavior`, `ReceiveBuilder`, `BehaviorBuilder`） | Rust に Java API は不要 |
| `Adapter`（classic/typed interop） | fraktor-rs は typed 優先設計。classic 互換層は不要 |
| `Deploy` / `Deployer` / `Scope` | JVM 固有のリモートデプロイ設定 |
| `AbstractFSM` / `AbstractLoggingFSM` | Java API。FsmBuilder で代替 |
| `ActorLogging` / `DiagnosticActorLogging` | Rust は tracing クレートで対応 |
| Classic `Stash` / `UnboundedStash` | typed 層の StashBuffer で統一 |
| Classic `Timers` trait | typed 層の TimerScheduler で統一 |
| `DynamicAccess` / `ReflectiveDynamicAccess` | JVM リフレクション固有 |
| `ClassicActorSystemProvider` / `ClassicActorContextProvider` | typed/classic ブリッジ。不要 |
| IO（TCP / UDP / DNS） | JVM NIO 固有。別モジュール or tokio で対応 |
| `LightArrayRevolverScheduler` | 実装詳細。fraktor-rs は TickDriver ベースの独自設計 |
| `IndirectActorProducer` | JVM ファクトリパターン |
| `FutureRef` / `PromiseRef` | classic パターン。typed では不要 |
| `BoundedDequeBasedMailbox` / `BoundedControlAwareMailbox` | Bounded 系の派生バリエーション。基本の Bounded + 機能組み合わせで対応可能 |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

該当なし。Phase 1 相当の項目はすべて実装済み。

### Phase 2: easy（単純な新規実装）

該当なし。Phase 2 相当の項目はすべて実装済み。

### Phase 3: medium（中程度の実装工数）

該当なし。Phase 3 相当の項目はすべて実装済み。

### Phase 4: hard（アーキテクチャ変更を伴う）

- `DurableProducerQueue` (core/typed + persistence) — persistence モジュール統合。永続化 + リプレイ + 再送の整合性設計が必要

### 対象外（n/a）

- Java API 重複、Classic 互換層、JVM 固有機能（上記 n/a テーブル参照）

## まとめ

- **全体カバレッジ 99%**: actor / actor-typed の主要 API はほぼ揃っており、classic kernel・typed wrapper・std adapter の主要機能は実装済み
- **前回からの実装完了項目**: `ByteString`, `CoordinatedShutdown`, `ProducerController`, `ConsumerController`, `WorkPullingProducerController` が追加され、前回 hard 判定だった大半の欠落が解消された
- **即座に価値を提供できる未実装機能**: なし。trivial / easy / medium は現時点で残っていない
- **実用上の主要ギャップ**: `DurableProducerQueue` のみ。typed reliable delivery と persistence を接続する最後の大型欠落で、永続化と再送の境界設計が必要
- **残りのギャップは Phase 4（hard）1件のみ**: 日常的な actor 利用、typed API、router、supervision、delivery、graceful shutdown まではほぼ完備している
- **補足**: `references/pekko/actor` / `actor-typed` 側には `DurableProducerQueue` が明示的に存在する一方、fraktor-rs 側には同名・同等の durable queue API は存在しないことを確認済み
