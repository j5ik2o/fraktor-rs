# actor モジュール ギャップ分析

## 前提

- 比較対象:
  - fraktor-rs core/kernel: `modules/actor-core/src/core/kernel/`
  - fraktor-rs core/typed: `modules/actor-core/src/core/typed/`
  - fraktor-rs std: `modules/actor-adaptor-std/src/std/`
  - Pekko classic: `references/pekko/actor/src/main/scala/org/apache/pekko/` (actor, dispatch, event, pattern, routing, serialization, io)
  - Pekko typed: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/`
- カバレッジ数値は `private` / `private[...]` / `internal` を除いた **主要公開契約** を型単位で数えたもの
- classic の Java 継承 DSL (`AbstractActor`, `ReceiveBuilder`, `AbstractActorWithTimers` 等) は JVM / Java モデル依存のため `n/a` 判定
- Java DSL 全般 (`javadsl/`, `japi/`) は `n/a` 判定
- Pekko IO パッケージ (`io/Tcp`, `io/Udp`, `io/Dns` 等) はネットワーク IO モジュールであり、fraktor-rs ではランタイム非依存の actor core に含めず、将来 remote / transport モジュールで扱うため `n/a` 判定
- 分析日: 2026-04-16（初版: 2026-04-15）

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（parity 対象） | 95 |
| fraktor-rs 対応実装数 | 92 |
| カバレッジ（型単位） | 92/95 (97%) |
| ギャップ数 | 3（core/kernel: 2, core/typed: 0, std: 1） |
| n/a 除外数 | 約 60（Java DSL, IO, japi, internal） |

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 35 | 33 | 94% |
| core / typed ラッパー | 54 | 54 | 100% |
| std / アダプタ | 6 | 5 | 83% |

`std` は Pekko の JVM 依存ランタイム補助（ロギング、スレッド実行器、協調停止、時計/回路遮断器相当）に対応づけている。

## カテゴリ別ギャップ

### classic actor core ✅ 実装済み 16/16 (100%)

ギャップなし。`PoisonPill`（`poison_pill.rs`）と `Kill`（`kill.rs`）が独立した公開 newtype として実装済み。いずれも `From<PoisonPill> for SystemMessage` / `From<Kill> for SystemMessage` 変換を提供。

実装済み型: `Actor` trait, `ActorCell`, `ActorContext`, `ActorPath`, `RootActorPath`, `ChildActorPath`, `ActorRef`, `DeadLetter`, `DeadLetterEntry`, `DeadLetterReason`, `DeadLetterShared`, `ActorIdentity`, `Identify`, `ActorSelection`, `Props`, `Address`, `ReceiveTimeout`, `PoisonPill`, `Kill`, `on_terminated` (Actor trait lifecycle hook)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `AbstractActor` / `ReceiveBuilder` | `AbstractActor.scala` | n/a | - | n/a | Java 継承 DSL。Rust では `Actor` trait と関数/クロージャで代替 |
| `AbstractActorWithTimers` 等 Java mixin 群 | `AbstractActor.scala`, `Timers.scala` | n/a | - | n/a | Java mixin API。意味的には `ActorContext::timers()` / `ClassicTimerScheduler` で代替 |

### supervision / fault handling ✅ 実装済み 8/8 (100%)

ギャップなし。`SupervisorStrategy`, `SupervisorStrategyKind` (OneForOne / AllForOne), `SupervisorDirective` (Resume/Restart/Stop/Escalate), `SupervisorStrategyConfig`, `RestartStatistics`, `BackoffSupervisorStrategy`, `BackoffOnFailureOptions`, `BackoffOnStopOptions`, `BackoffSupervisor` は主要契約をカバー。

### typed core surface ✅ 実装済み 36/36 (100%)

ギャップなし。前回分析時に未対応・部分実装としていた以下の4型がすべて独立した公開型として実装済みであることを確認:

- `ExtensibleBehavior`（`extensible_behavior.rs`）: `receive` と `receive_signal` メソッドを持つ公開 trait。`Behaviors::from_extensible` で `Behavior` に変換可能
- `Terminated`（`message_and_signals/terminated.rs`）: `TypedActorRef<Infallible>` を保持する独立 struct。`Signal` trait と `From<Terminated> for BehaviorSignal` を実装
- `ChildFailed`（`message_and_signals/child_failed.rs`）: `Terminated` + `ActorError` を保持する独立 struct。Pekko と同様に `Terminated` のサブタイプ関係を合成で表現
- `MessageAdaptionFailure`（`message_and_signals/message_adaption_failure.rs`）: `AdapterError` を保持する独立 struct。`Signal` trait と `From<MessageAdaptionFailure> for BehaviorSignal` を実装

実装済み型: `Behavior`, `Receive`, `Behaviors` (setup/receive/receiveMessage/withTimers/withStash/logMessages/withMdc/intercept/transformMessages/monitor/stopped), `TypedActorContext`, `TypedActorRef`, `TypedActorSystem`, `ActorRefResolver`, `AbstractBehavior` trait, `ExtensibleBehavior` trait, `BehaviorInterceptor`, `BehaviorSignalInterceptor`, `BehaviorSignal`, `Terminated`, `ChildFailed`, `MessageAdaptionFailure`, `PreRestart`, `PostStop`, `DeathPactError`, `Signal` trait, `LogOptions`, `DispatcherSelector`, `MailboxSelector`, `TypedProps`, `ActorTags`, `SpawnProtocol`, `RecipientRef`, `MessageAdapterRegistry`, `AdapterPayload`, `TypedAskFuture`, `TypedAskResponse`, `StatusReply`, `FsmBuilder`, `BackoffSupervisorStrategy`, `RestartSupervisorStrategy`, `SupervisorStrategy` (typed)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `BehaviorBuilder` (Java DSL) | `javadsl/BehaviorBuilder.scala` | n/a | - | n/a | Java DSL 専用 builder |
| `ReceiveBuilder` (Java DSL) | `javadsl/ReceiveBuilder.scala` | n/a | - | n/a | Java DSL 専用 builder |
| `AbstractMatchingBehavior` (Java DSL) | `javadsl/AbstractMatchingBehavior.scala` | n/a | - | n/a | Java DSL 専用 |

### dispatch / mailbox ✅ 実装済み 12/13 (92%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `NonBlockingBoundedMailbox` | `Mailbox.scala:L688` | 未対応 | core/kernel | easy | backpressure ベースの bounded mailbox。既存 `BoundedMessageQueue` のバリエーション |

実装済み型: `Mailbox`, `MessageQueue` trait, `MailboxType` trait, `Envelope`, `UnboundedMessageQueue`, `BoundedMessageQueue`, `UnboundedDequeMessageQueue`, `UnboundedPriorityMessageQueue`, `BoundedPriorityMessageQueue`, `UnboundedStablePriorityMessageQueue`, `BoundedStablePriorityMessageQueue`, `UnboundedControlAwareMessageQueue`, `UnboundedControlAwareMailboxType`, `MessagePriorityGenerator` trait, `MailboxCapacity`, `MailboxOverflowStrategy`, `MailboxPolicy`, `Mailboxes`, `Dispatchers`, `DefaultDispatcher`, `PinnedDispatcher`, `BalancingDispatcher`, `Executor` trait, `InlineExecutor`, `MessageDispatcher` trait, `DispatcherSettings`, `SharedMessageQueue`

### event / logging ✅ 実装済み 8/9 (89%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `LoggingFilter` / `LoggingFilterWithMarker` | `Logging.scala:L1572-1604` | 未対応 | core/kernel | medium | ログレベルフィルタリング機構。現在の `LoggerWriter` trait / `LoggingAdapter` にフィルタ概念がない |

実装済み型: `EventStream`, `EventStreamSubscriber` trait, `EventStreamSubscription`, `LogEvent`, `LogLevel`, `LoggingAdapter`, `BusLogging`, `NoLogging`, `ActorLogging`, `DiagnosticActorLogging`, `ActorLogMarker`, `LoggingReceive`, `LoggerSubscriber` (core), `TracingLoggerSubscriber` / `DeadLetterLogSubscriber` (std)

備考: Pekko の `EventBus` trait（EventStream とは別の汎用イベントバス抽象）は未実装だが、fraktor では `EventStreamSubscriber` trait が同等の役割を果たしており、実質的な機能欠落はない。独立した汎用 `EventBus` trait の必要性は低い。

### pattern ✅ 実装済み 5/5 (100%)

ギャップなし。前回分析時に未対応としていた `CircuitBreakersRegistry` が `modules/actor-adaptor-std/src/std/pattern/circuit_breakers_registry.rs` に実装済みであることを確認。`Extension` trait を実装し、`from_actor_system` / `get` / `with_named_config` 等のメソッドで名前ベースの CB インスタンス管理を提供。

実装済み型: `CircuitBreaker`, `CircuitBreakerShared`, `CircuitBreakerState`, `CircuitBreakerOpenError`, `CircuitBreakerCallError`, `Clock` trait, `CircuitBreakersRegistry`, `ask_with_timeout`, `graceful_stop`, `graceful_stop_with_message`, `retry`, `pipe_to` / `pipe_to_self` (ActorContext メソッド)

### classic routing ✅ 実装済み 6/9 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ConsistentHashingRoutingLogic` (classic kernel) | `ConsistentHashing.scala:L158` | 未対応 | core/kernel | medium | typed `PoolRouter::with_consistent_hash` / `GroupRouter::with_consistent_hash` は存在するが、kernel 層の独立した `RoutingLogic` 実装がない |
| `SmallestMailboxRoutingLogic` (classic kernel) | `SmallestMailbox.scala:L48` | 未対応 | core/kernel | medium | typed `PoolRouter::with_smallest_mailbox` は存在するが、kernel 層の独立した `RoutingLogic` 実装がない |
| classic `Pool` / `Group` router config infrastructure | `RouterConfig.scala:L144-266` | 未対応 | core/kernel | hard | Pekko classic の `RouterConfig`, `Pool`, `Group`, `CustomRouterConfig`, `FromConfig` 等の設定駆動ルータ基盤。typed 側は `PoolRouter` / `GroupRouter` で代替済み |

実装済み型 (kernel): `RoutingLogic` trait, `Router`, `Routee`, `Broadcast`, `RandomRoutingLogic`, `RoundRobinRoutingLogic`, `RouterCommand`, `RouterResponse`

### typed routing ✅ 実装済み 6/6 (100%)

ギャップなし。`Routers`, `PoolRouter`, `GroupRouter`, `BalancingPoolRouterBuilder`, `ScatterGatherFirstCompletedRouterBuilder`, `TailChoppingRouterBuilder`, `DefaultResizer`, `Resizer` trait は主要契約をカバー。ConsistentHash / SmallestMailbox は `PoolRouter` / `GroupRouter` のメソッドとして利用可能。

### discovery / receptionist ✅ 実装済み 9/9 (100%)

ギャップなし。`Receptionist`, `ServiceKey`, `Register`, `Deregister`, `Subscribe`, `Find`, `Listing`, `Registered`, `Deregistered` は主要契約をカバー。

### scheduling / timers ✅ 実装済み 8/8 (100%)

ギャップなし。classic `Scheduler` / `ClassicTimerScheduler` / `Cancellable` (`= SchedulerHandle`)、typed `Scheduler` / `TimerScheduler` / `TimerKey` は実装済み。

### ref / resolution ✅ 実装済み 6/6 (100%)

ギャップなし。`ActorRef`, `ActorSelection`, `ActorPath`, `ActorRefResolver`, `narrow`, `unsafe_upcast`, `to/from serialization format` まで揃っている。

### delivery / pubsub ✅ 実装済み 8/8 (100%)

ギャップなし。`ProducerController`, `ConsumerController`, `DurableProducerQueue`, `Topic`, `TopicStats`, `WorkPullingProducerController`, `SequencedMessage`, `WorkerStats` まで揃っている。

### serialization ✅ 実装済み 8/8 (100%)

ギャップなし。`Serializer` trait, `SerializerWithStringManifest`, `ByteBufferSerializer`, `AsyncSerializer`, `SerializationExtension`, `SerializationRegistry`, `SerializationSetup`, `SerializedMessage`, `SerializerId`, `SerializationDelegator`, builtin serializers (Bool/ByteString/Bytes/I32/Null/String) まで揃っている。Pekko の `JavaSerializer` / `DisabledJavaSerializer` は JVM 固有のため n/a。

### extension ✅ 実装済み 4/4 (100%)

ギャップなし。`Extension` trait, `ExtensionId` trait, `ExtensionInstaller` trait, `ExtensionInstallers` は実装済み。typed 側も `ExtensionSetup`, `AbstractExtensionSetup` を提供。

### coordinated shutdown ✅ 実装済み 5/5 (100%)

ギャップなし。`CoordinatedShutdown`, `CoordinatedShutdownPhase`, `CoordinatedShutdownReason`, `CoordinatedShutdownInstaller`, `CoordinatedShutdownId` は実装済み。

### std adaptor ✅ 実装済み 5/6 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `AffinityPool` executor | `dispatch/affinity/AffinityPool.scala` | 未対応 | std | hard | CPU affinity ベースのスレッドプール。Rust では tokio の work-stealing が同等の役割を担うが、独立した affinity executor は未実装 |

`VirtualThreadExecutorConfigurator` は JVM 固有（Java 21+ 仮想スレッド）であり、Rust では tokio / smol が同等のスケジューリングを提供するため `n/a` に再分類。

実装済み型 (std): `TokioExecutor`, `TokioExecutorFactory`, `PinnedExecutor`, `PinnedExecutorFactory`, `ThreadedExecutor`, `StdClock`, `StdBlocker`, `TracingLoggerSubscriber`, `DeadLetterLogSubscriber`, `StdTickDriver`, `TokioTickDriver`

## 内部モジュール構造ギャップ

API ギャップが 97% まで詰まっており、主要カテゴリの致命的欠落がないため、内部構造ギャップも分析対象に含める。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|----------------|--------|--------|------|
| receptionist の facade / protocol / runtime 実装がまだ粗く同居 | `actor-typed/receptionist/Receptionist.scala`, `actor-typed/internal/receptionist/ReceptionistMessages.scala` | `core/typed/receptionist.rs` が facade + behavior を保持し、protocol 型だけ `receptionist/` 配下に分割 | `core/typed/receptionist/` に behavior 実装も寄せ、公開 facade と内部実装の境界を明確化 | medium | high | 今後 serializer / cluster receptionist 拡張を入れると 1 ファイル集中が重くなる |
| typed delivery に `internal` 層がなく、公開型と制御ロジックが同じ階層に並ぶ | `actor-typed/delivery/*`, `actor-typed/delivery/internal/ProducerControllerImpl.scala` | `core/typed/delivery/` 直下に command / settings / behavior / state が並列 | `delivery/internal/` を新設し、controller 実装詳細と公開 DTO を分離 | medium | medium | 現時点で API は揃っているが、再送・永続キュー拡張時に責務が散りやすい |
| classic kernel の public surface が広く、内部補助型まで `pub` に露出しやすい | Pekko classic は package-private / internal API が多い | `core/kernel/**` に利用者向けでない `pub` 型が広く存在 | `pub(crate)` へ寄せられるものを継続的に縮小し、入口 facade からの再公開を基準に露出制御 | medium | medium | fraktor は `pub` 露出が多く、型数だけで見ると Pekko を上回る |
| classic routing の kernel 層に ConsistentHash / SmallestMailbox RoutingLogic がない | Pekko `routing/ConsistentHashing.scala`, `routing/SmallestMailbox.scala` | typed `PoolRouter` / `GroupRouter` 上のメソッドとしては存在するが、kernel `RoutingLogic` 実装がない | `core/kernel/routing/` に `ConsistentHashRoutingLogic` と `SmallestMailboxRoutingLogic` を追加 | medium | medium | typed 層から kernel 層へロジックを降ろす構造変更 |

## 実装優先度

### Phase 1（trivial / easy）

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| `NonBlockingBoundedMailbox` | core/kernel | 既存 `BoundedMessageQueue` のバリエーション追加 |

### Phase 2（medium）

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| `ConsistentHashingRoutingLogic` (kernel) | core/kernel | kernel 層への独立 RoutingLogic 追加と ConsistentHash ユーティリティ |
| `SmallestMailboxRoutingLogic` (kernel) | core/kernel | kernel 層への独立 RoutingLogic 追加とメールボックスサイズ取得 API |
| `LoggingFilter` / `LoggingFilterWithMarker` | core/kernel | ロガー初期化にフィルタ概念を追加 |
| receptionist 実装の `receptionist/` 配下への再配置 | core/typed | API を壊さず責務を整理できるが、ファイル分割は複数箇所に波及する |
| delivery の `internal` 分離 | core/typed | 既存 controller 群の責務整理が必要 |

### Phase 3（hard）

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| classic `Pool` / `Group` router config infrastructure | core/kernel | 設定駆動ルーティング基盤の新規設計。`RouterConfig`, `Pool`, `Group`, `CustomRouterConfig`, `FromConfig` 相当 |
| `AffinityPool` executor | std | CPU affinity ベースのスレッドプール。低レベルスケジューリング |

### 対象外（n/a）

| 項目 | 理由 |
|------|------|
| `AbstractActor` / `ReceiveBuilder` 等 Java 継承 DSL | JVM / Java 継承モデル依存。Rust の `Actor` trait + closure で代替 |
| `AbstractActorWithTimers` 等 Java mixin 群 | JVM / Java mixin 依存。`ClassicTimerScheduler` / typed `TimerScheduler` でカバー |
| `BehaviorBuilder` / `ReceiveBuilder` (Java DSL) | Java DSL 専用 |
| `AbstractMatchingBehavior` (Java DSL) | Java DSL 専用 |
| IO パッケージ (`Tcp`, `Udp`, `Dns` 等) | ネットワーク IO は remote / transport モジュールで扱う。actor core の parity 対象外 |
| `JavaSerializer` / `DisabledJavaSerializer` | JVM Java シリアライゼーション固有 |
| `japi/` パッケージ全体 | Java API interop 層 |
| `VirtualThreadExecutorConfigurator` / `VirtualizedExecutorService` | JVM 固有（Java 21 仮想スレッド）。Rust では tokio が同等 |
| `DynamicAccess` / `ReflectiveDynamicAccess` | JVM クラスローダー/リフレクション固有 |
| `IndirectActorProducer` / `TypedCreatorFunctionConsumer` | JVM クラスベースの Actor 生成。Rust では `Props` + closure で代替 |
| `ProviderSelection` | JVM の ActorSystem プロバイダ選択機構。fraktor-rs では不要 |

## まとめ

- actor モジュールの parity は **97%** に達しており、前回分析（89%）から大幅に向上した。`PoisonPill`/`Kill` 公開型、`ExtensibleBehavior` trait、`Terminated`/`ChildFailed`/`MessageAdaptionFailure` 独立 signal 型、`CircuitBreakersRegistry` Extension の計7型が実装済みであることを確認した。
- **完全カバー済みカテゴリ**（100%）: classic actor core, supervision, typed core surface, typed routing, receptionist, scheduling/timers, ref/resolution, delivery/pubsub, serialization, extension, coordinated shutdown, pattern — **12カテゴリ**で完全 parity。
- 残存 API ギャップは **3件** のみ:
  - `NonBlockingBoundedMailbox`（Phase 1, core/kernel, easy）
  - `LoggingFilter`（Phase 2, core/kernel, medium）
  - classic routing の kernel 層 RoutingLogic 不足（`ConsistentHash`/`SmallestMailbox`）+ `RouterConfig` 設定駆動基盤（Phase 2-3, core/kernel）
- `AffinityPool` executor（Phase 3, std, hard）は残存するが、tokio の work-stealing が同等機能を提供しており優先度は低い。
- 次のボトルネックは API 不足そのものよりも、**receptionist / delivery の内部責務の切り方**、および **kernel 層の `pub` 露出過多** にある。構造整理を並行して進めることで、以後の parity 実装速度を維持できる。
