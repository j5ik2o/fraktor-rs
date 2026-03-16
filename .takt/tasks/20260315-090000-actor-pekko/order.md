# actor モジュール ギャップ分析

Pekko互換仕様を実装する必要があります。
Phase 4: hard（アーキテクチャ変更を伴う）は対象外です。Phase 1から3を必ず実装してください。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（意味のある型単位） | 約 110（classic: ~65, typed: ~45） |
| fraktor-rs 公開型数 | 約 95（core/kernel: ~45, core/typed: ~35, std: ~15） |
| カバレッジ（型単位） | 約 85/110 (77%) |
| ギャップ数 | 25（core: 10, std: 5, n/a: 10） |

※ Java API 重複（AbstractBehavior, ReceiveBuilder, javadsl.* 等）、private[pekko] 内部型、JVM 固有機能（Deploy, IO, DynamicAccess 等）は Pekko 側計数から除外。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | ~50 | ~45 | 90% |
| core / typed ラッパー | ~45 | ~35 | 78% |
| std / アダプタ | （実装固有） | ~15 | — |

## カテゴリ別ギャップ

### コア型（ActorRef, ActorSystem, Props, ActorContext） ✅ 実装済み 12/12 (100%)

全主要型が実装済み。ギャップなし。
ActorRef, ActorSystem, ExtendedActorSystem, Props, ActorContext, ActorCell, ActorRefProvider, ActorRefFactory 相当がすべて存在。

### Address / ActorPath ✅ 実装済み 3/5 (60%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RootActorPath` | `ActorPath.scala:278` | 未対応 | core/kernel | easy | ActorPath にルート/子の区別がない。単一 struct で表現中 |
| `ChildActorPath` | `ActorPath.scala:327` | 未対応 | core/kernel | easy | 同上。型レベルの区別が必要かは設計判断 |

### メッセージ型・シグナル ✅ 実装済み 11/14 (79%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `PoisonPill` | `Actor.scala:46` | 未対応 | core/kernel | easy | graceful_stop で代替可能。SystemMessage に追加する形 |
| `Kill` | `Actor.scala:60` | 未対応 | core/kernel | easy | 即時停止シグナル。SystemMessage に追加 |
| `DeathPactException` | `MessageAndSignals.scala:130` | 未対応 | core/typed | easy | death pact のハンドリングは BehaviorRunner に存在するが、明示的なエラー型がない |

実装済み: Identify, ActorIdentity, Terminated（BehaviorSignal::Terminated）, ReceiveTimeout, Status, UnhandledMessage（UnhandledMessageEvent）, PreRestart（BehaviorSignal::PreRestart）, PostStop（BehaviorSignal::Stopped + TypedActor::post_stop）, ChildFailed（BehaviorSignal::ChildFailed）, MessageAdaptionFailure（BehaviorSignal::MessageAdaptionFailure）, Signal（BehaviorSignal）

### Dead Letters ✅ 実装済み 3/3 (100%)

全主要型が実装済み。DeadLetter, SuppressedDeadLetter（DeadLetterReason::SuppressedDeadLetter）, Dropped（DeadLetterReason::Dropped）が実装済み。

### Behaviors ファクトリ ✅ 実装済み 14/16 (88%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Behavior.narrow` | `Behavior.scala:104` | 未対応 | core/typed | medium | メッセージ型の反変変換。Rust の型システムでは trait bound で表現が必要 |
| `Behaviors.transformMessages` | `BehaviorImpl.scala` | 未対応 | core/typed | medium | 外部メッセージ型から内部メッセージ型への変換ラッパー |

実装済み: setup, receive_message, receive_and_reply, receive_signal, same, stopped, unhandled, empty, ignore, with_stash, supervise, log_messages / log_messages_with_opts, monitor, with_timers, intercept（BehaviorInterceptor経由）, with_mdc / with_static_mdc

※ `receivePartial` / `receiveMessagePartial` は Scala の PartialFunction 固有であり n/a。`receiveMessageWithSame` は `receive_message` + `Behaviors::same()` で実現可能。

### Supervision ✅ 実装済み 5/5 (100%)

全主要型が実装済み。SupervisorStrategy（resume/restart/stop）, BackoffSupervisorStrategy, Supervise ビルダー, OneForOneStrategy（SupervisorStrategyKind::OneForOne）, AllForOneStrategy（SupervisorStrategyKind::AllForOne）が実装済み。SupervisorDirective, RestartStatistics も存在。

### BehaviorInterceptor ✅ 実装済み 2/2 (100%)

BehaviorInterceptor, BehaviorSignalInterceptor が実装済み。Pekko の PreStartTarget / ReceiveTarget / SignalTarget は Rust のクロージャで暗黙的に対応。

### ActorContext (typed) ✅ 実装済み 13/14 (93%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorContext.scheduleOnce` | `ActorContext.scala:191` | 未対応 | core/typed | easy | TypedScheduler 経由で間接的に可能だが、context 上の直接メソッドがない |

実装済み: self_ref, system, spawn_child, spawn_child_watched, stop_self, stop_child, watch, watch_with, unwatch, children, child, message_adapter_builder, delegate, pipe_to_self, set_receive_timeout, ask

### Receptionist / ServiceKey ✅ 実装済み 7/7 (100%)

全主要型・コマンドが実装済み。Receptionist, ServiceKey, Listing, ReceptionistCommand（Register, Deregister, Subscribe, Unsubscribe）が実装済み。

### Router ✅ 実装済み 7/11 (64%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ScatterGatherFirstCompleted` | `ScatterGatherFirstCompleted.scala` | 未対応 | core/typed | medium | 全ルーティーに送信し最初の応答を返す。タイムアウト付き |
| `TailChopping` | `TailChopping.scala` | 未対応 | core/typed | medium | 順次送信して最初の応答を返す。レイテンシ最適化パターン |
| `BalancingPool` | `Balancing.scala` | 未対応 | core/typed | hard | 共有メールボックスベース。SmallestMailbox で近似可能 |
| `Resizer` | `Resizer.scala` | 未対応 | core/typed | medium | ルーター配下のルーティー数を動的に調整。負荷に応じた自動スケーリング |

実装済み: Routers, PoolRouterBuilder, GroupRouterBuilder, RoundRobin, Random, Broadcast, ConsistentHashing, SmallestMailbox, BroadcastPredicate

### Timer / Stash ✅ 実装済み 4/4 (100%)

TimerScheduler（startTimerWithFixedDelay, startTimerAtFixedRate, startSingleTimer, isTimerActive, cancel, cancelAll）, TimerKey, StashBuffer が実装済み。

### SpawnProtocol ✅ 実装済み 1/1 (100%)

SpawnProtocol が `core/typed/spawn_protocol.rs` に実装済み。

### LogOptions ✅ 実装済み 1/1 (100%)

LogOptions が `std/typed/log_options.rs` に実装済み（withEnabled, withLevel, withLoggerName）。

### ActorRefResolver ✅ 実装済み 1/1 (100%)

ActorRefResolver が `core/typed/actor_ref_resolver.rs` に実装済み。

### Ask パターン ✅ 実装済み 4/4 (100%)

ask_with_timeout, ask on context, pipe_to_self, StatusReply が実装済み。TypedAskFuture, TypedAskResponse も存在。

### EventStream ✅ 実装済み 3/3 (100%)

EventStream（subscribe, unsubscribe, publish）が実装済み。EventStreamShared, EventStreamSubscription も存在。

### Topic / PubSub ✅ 実装済み 5/5 (100%)

Topic, TopicCommand（Publish, Subscribe, Unsubscribe, GetTopicStats）, TopicStats が実装済み。

### Extension ✅ 実装済み 4/4 (100%)

Extension, ExtensionId, ExtensionInstaller, ExtensionSetup が実装済み。

### FSM ✅ 実装済み 1/1 (100%)

FsmBuilder が typed 層に存在。Pekko の classic FSM は複雑な Scala DSL だが、fraktor-rs は typed 層で簡潔に実装。

### Dispatch / Mailbox ✅ 実装済み 5/12 (42%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `PinnedDispatcher` | `PinnedDispatcher.scala` | 未対応 | std | medium | 1アクター1スレッド専用ディスパッチャ。ブロッキング処理用 |
| `UnboundedPriorityMailbox` | `Mailbox.scala` | 未対応 | core/kernel | medium | 優先度付きメッセージキュー |
| `BoundedPriorityMailbox` | `Mailbox.scala` | 未対応 | core/kernel | medium | 容量制限+優先度付きキュー |
| `UnboundedStablePriorityMailbox` | `Mailbox.scala` | 未対応 | core/kernel | medium | 安定ソート保証付き優先度キュー |
| `BoundedStablePriorityMailbox` | `Mailbox.scala` | 未対応 | core/kernel | medium | 容量制限+安定優先度キュー |
| `UnboundedDequeBasedMailbox` | `Mailbox.scala` | 未対応 | core/kernel | easy | 両端キュー。Stash 実装で利用 |
| `UnboundedControlAwareMailbox` | `Mailbox.scala` | 未対応 | core/kernel | medium | 制御メッセージ優先キュー |

実装済み: MailboxType, MessageQueue, UnboundedMailbox, BoundedMailbox, MailboxCapacity, MailboxOverflowStrategy, MailboxInstrumentation, Dispatchers, DispatcherConfig, DispatchExecutor

### Serialization ✅ 実装済み 4/6 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ByteBufferSerializer` | `Serializer.scala:195` | 未対応 | core/kernel | easy | ByteBuffer ベースの効率的シリアライゼーション。zero-copy 最適化 |
| `AsyncSerializer` | `AsyncSerializer.scala` | 未対応 | core/kernel | medium | 非同期シリアライゼーション。大きなメッセージや外部ストレージ連携 |

実装済み: Serializer, SerializerWithStringManifest, SerializationRegistry, SerializationExtension, SerializationSetup, SerializationDelegator, 組み込みシリアライザ（Bool, I32, Null, String, Bytes）

### Pattern（classic ユーティリティ） ✅ 実装済み 4/7 (57%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `CircuitBreaker` | `CircuitBreaker.scala` | 未対応 | std | medium | サーキットブレーカー。外部サービス呼び出しの障害分離 |
| `PipeToSupport` (classic) | `PipeToSupport.scala` | 別名で実装済み | — | — | typed の `pipe_to_self` で代替。classic API は不要 |
| `BackoffSupervisor` (classic actor) | `BackoffSupervisor.scala` | 別名で実装済み | — | — | typed の BackoffSupervisorStrategy で代替 |

※ `FutureRef` / `PromiseRef` は classic パターンで typed では不要（n/a）。

実装済み: ask_with_timeout, graceful_stop, graceful_stop_with_message, retry, StatusReply, pipe_to_self（typed context 上）

### CoordinatedShutdown ❌ 未対応 0/1

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `CoordinatedShutdown` | `CoordinatedShutdown.scala:41` | 未対応 | std | hard | フェーズ付きシャットダウンオーケストレーション。ActorSystem 終了時のリソース解放順序制御。各フェーズにタスクを登録し、依存関係を保証しながら順次実行する |

### Reliable Delivery（信頼性メッセージング） ❌ 未対応 0/4

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ProducerController` | `ProducerController.scala:60` | 未対応 | core/typed | hard | プロデューサー側のフロー制御。SeqNr ベースの確認応答 |
| `ConsumerController` | `ConsumerController.scala:60` | 未対応 | core/typed | hard | コンシューマー側のフロー制御。ウィンドウベースの再送 |
| `WorkPullingProducerController` | `WorkPullingProducerController.scala` | 未対応 | core/typed | hard | ワークプル型の信頼性メッセージング。複数ワーカーへの分散 |
| `DurableProducerQueue` | `DurableProducerQueue.scala:33` | 未対応 | core/typed + std | hard | 永続化キュー。persistence モジュールとの統合が必要 |

### Util（ユーティリティ型） ❌ 未対応 0/3

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ByteString` | `ByteString.scala` | 未対応 | core/kernel | medium | イミュータブルバイト列。zero-copy スライス・結合。IO/serialization の基盤型 |
| `MessageBuffer` | `MessageBuffer.scala` | 未対応 | core/typed | easy | メッセージのバッファリング。Stash に似るがより軽量 |
| `MessageBufferMap` | `MessageBuffer.scala` | 未対応 | core/typed | easy | キー別メッセージバッファ |

### その他の typed API ❌ 未対応 0/2

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorTags` | `Props.scala:144` | 未対応 | core/typed | easy | アクターにタグを付与。監視・デバッグ用メタデータ |
| `Behavior.transformMessages` | `BehaviorImpl.scala` | 未対応 | core/typed | medium | メッセージ型変換ラッパー。messageAdapter の静的版 |

### 対象外（n/a）

| Pekko API | 理由 |
|-----------|------|
| Java API 重複（`javadsl.*`, `AbstractBehavior`, `ReceiveBuilder`, `BehaviorBuilder`） | Rust に Java API は不要 |
| `Adapter`（classic/typed interop） | fraktor-rs は typed 優先設計。classic 互換層は不要 |
| `Deploy` / `Deployer` / `Scope` | JVM 固有のリモートデプロイ設定 |
| `AbstractFSM` / `AbstractLoggingFSM` | Java API。FSMBuilder で代替 |
| `ActorLogging` / `DiagnosticActorLogging` | Rust は tracing クレートで対応 |
| Classic `Stash` / `UnboundedStash` | typed 層の StashBuffer で統一 |
| Classic `Timers` trait | typed 層の TimerScheduler で統一 |
| `DynamicAccess` / `ReflectiveDynamicAccess` | JVM リフレクション固有 |
| `ClassicActorSystemProvider` / `ClassicActorContextProvider` | typed/classic ブリッジ。不要 |
| IO（TCP / UDP / DNS） | JVM NIO 固有。別モジュール or tokio で対応 |
| `receivePartial` / `receiveMessagePartial` | Scala PartialFunction 固有 |
| `LightArrayRevolverScheduler` | 実装詳細。fraktor-rs は TickDriver ベースの独自設計 |
| `IndirectActorProducer` | JVM ファクトリパターン |
| `FutureRef` / `PromiseRef` | classic パターン。typed では不要 |
| `BoundedDequeBasedMailbox` / `BoundedControlAwareMailbox` | Bounded 系の派生バリエーション。基本の Bounded + 機能組み合わせで対応可能 |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- `ActorTags` (core/typed) — Props にタグ Set を追加するだけ
- `MessageBuffer` / `MessageBufferMap` (core/typed) — Vec ベースのシンプルなバッファ。StashBuffer の簡易版

### Phase 2: easy（単純な新規実装）

- `PoisonPill` / `Kill` (core/kernel) — SystemMessage に variant 追加 + ActorCell でのハンドリング
- `DeathPactException` (core/typed) — ActorError に variant 追加
- `RootActorPath` / `ChildActorPath` (core/kernel) — ActorPath を enum 化して型区別を導入
- `ByteBufferSerializer` (core/kernel) — Serializer trait のバリエーション追加
- `UnboundedDequeBasedMailbox` (core/kernel) — VecDeque ベースの MessageQueue 実装
- `ActorContext.scheduleOnce` (core/typed) — TypedScheduler の start_single_timer への委譲

### Phase 3: medium（中程度の実装工数）

- `Behavior.narrow` / `transformMessages` (core/typed) — 型変換ラッパー Behavior。ジェネリクス設計が必要
- `CircuitBreaker` (std) — 3状態（Closed/Open/HalfOpen）の状態機械 + タイマー連携
- `PinnedDispatcher` (std) — 専用スレッド/タスクへのディスパッチ。tokio::task::spawn_blocking 活用
- `PriorityMailbox` 群 (core/kernel) — BinaryHeap ベースの MessageQueue 実装。MailboxType の拡張
- `ControlAwareMailbox` (core/kernel) — 2キュー（制御用 + 通常用）の MessageQueue 実装
- `AsyncSerializer` (core/kernel) — async fn serialize/deserialize を持つ trait
- `ScatterGatherFirstCompleted` (core/typed) — 全ルーティーへの ask + select!/race
- `TailChopping` (core/typed) — 順次送信 + タイムアウト付き応答待ち
- `Resizer` (core/typed) — PoolRouter の動的サイズ調整。メトリクス連携

### Phase 4: hard（アーキテクチャ変更を伴う）

- `CoordinatedShutdown` (std) — フェーズ付きシャットダウン。ActorSystem のライフサイクルに深く関わる。フェーズ依存関係グラフの管理、タイムアウト、理由追跡が必要
- `BalancingPool` (core/typed + std) — 共有メールボックスの設計が必要。現在の 1アクター1メールボックス の前提に影響
- `ProducerController` (core/typed) — SeqNr ベースの確認応答プロトコル。フロー制御の状態機械
- `ConsumerController` (core/typed) — ウィンドウベースの再送プロトコル。フロー制御 + SequencedMessage
- `WorkPullingProducerController` (core/typed) — ワークプル型分散。Receptionist 連携 + 動的ワーカー管理
- `DurableProducerQueue` (core/typed + std) — persistence モジュール統合。永続化 + リプレイ
- `ByteString` (core/kernel) — イミュータブルバイト列。bytes クレートの Bytes 型で代替可能だが、Pekko 互換 API が必要な場合は独自実装

### 対象外（n/a）

- Java API 重複、Classic 互換層、JVM 固有機能（上記 n/a テーブル参照）

## まとめ

- **全体カバレッジ 77%**: 主要な typed API（Behavior, ActorContext, Supervision, Receptionist, Router, Ask, Timer, Stash, Topic, SpawnProtocol, Extension, EventStream, ActorRefResolver, LogOptions, FSM）はほぼ完全にカバー済み。前回分析時から ChildFailed, MessageAdaptionFailure, Behaviors.withTimers, SpawnProtocol, ActorRefResolver, Deregister, EventStream.unsubscribe, GetTopicStats など多数の機能が実装された
- **即座に価値を提供できる未実装機能**: `ActorTags`（Phase 1）はデバッグ・監視に有用。`MessageBuffer`（Phase 1）は軽量なメッセージ保持に便利。`PoisonPill`/`Kill`（Phase 2）は Pekko 利用者にとって馴染みのある停止 API
- **実用上の主要ギャップ**: `CoordinatedShutdown`（Phase 4）は本番運用で最も重要。`CircuitBreaker`（Phase 3）は外部サービス連携で必須。Reliable Delivery（Phase 4）は高信頼メッセージングに必要だが persistence モジュールの成熟が前提。`PriorityMailbox` / `ControlAwareMailbox`（Phase 3）は高度なメッセージ処理制御に必要
- **Dispatch/Mailbox が最大のギャップ領域**: カバレッジ 42% で、優先度・デキュー・制御対応など高度なメールボックスバリエーションが不足。ただし基本の Unbounded/Bounded は実装済みで、多くのユースケースをカバーしている
