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

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（parity 対象） | 95 |
| fraktor-rs 対応実装数 | 81 |
| カバレッジ（型単位） | 81/95 (85%) |
| ギャップ数 | 14（core/kernel: 5, core/typed: 7, std: 2） |
| n/a 除外数 | 約 60（Java DSL, IO, japi, internal） |

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 35 | 30 | 86% |
| core / typed ラッパー | 54 | 47 | 87% |
| std / アダプタ | 6 | 4 | 67% |

`std` は Pekko の JVM 依存ランタイム補助（ロギング、スレッド実行器、協調停止、時計/回路遮断器相当）に対応づけている。

## カテゴリ別ギャップ

### classic actor core ✅ 実装済み 14/16 (88%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `AbstractActor` / `ReceiveBuilder` | `AbstractActor.scala` | n/a | - | n/a | Java 継承 DSL。Rust では `Actor` trait と関数/クロージャで代替 |
| `AbstractActorWithTimers` 等 Java mixin 群 | `AbstractActor.scala`, `Timers.scala` | n/a | - | n/a | Java mixin API。意味的には `ActorContext::timers()` / `ClassicTimerScheduler` で代替 |
| `ActorSystem.registerOnTermination` | `ActorSystem.scala:L570` | 未対応 | core/kernel | easy | `when_terminated` はあるが callback 登録 API はない。`TerminationSignal` に `on_terminated(callback)` を追加で実現可 |
| `PoisonPill` / `Kill` の classic 公開 surface | `Actor.scala:L46-67` | 部分実装 | core/kernel | easy | 内部 `SystemMessage::{PoisonPill,Kill}` は存在するが、独立した公開型としての surface が不足。classic `PoisonPill` / `Kill` 型が公開されていない |

実装済み型: `Actor` trait, `ActorCell`, `ActorContext`, `ActorPath`, `RootActorPath`, `ChildActorPath`, `ActorRef`, `DeadLetter`, `ActorIdentity`, `Identify`, `ActorSelection`, `Props`, `Address`, `ReceiveTimeout`, `Stash` (kernel 側 `MessageBuffer`/`MessageBufferMap`)

### supervision / fault handling ✅ 実装済み 7/8 (88%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| classic `AllForOneStrategy` | `FaultHandling.scala:L465` | 未対応 | core/kernel | medium | 現在は `OneForOne` 相当の `SupervisorStrategy` のみ。`AllForOne` は兄弟アクターの一括再起動制御が必要 |

実装済み型: `SupervisorStrategy`, `SupervisorDirective` (Resume/Restart/Stop/Escalate), `SupervisorStrategyConfig` (OneForOne), `RestartStatistics`, `BackoffSupervisorStrategy`, `BackoffOnFailureOptions`, `BackoffOnStopOptions`, `BackoffSupervisor`

### typed core surface ✅ 実装済み 31/36 (86%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ExtensibleBehavior` | `Behavior.scala:L106` | 未対応 | core/typed | easy | `Behavior` と `BehaviorInterceptor` はあるが、`ExtensibleBehavior` 相当の公開 abstract base type が分離されていない。`AbstractBehavior` trait は存在するが `ExtensibleBehavior` とは役割が異なる |
| `Terminated` 公開 signal 型 | `MessageAndSignals.scala:L81` | 部分実装 | core/typed | easy | 現在は `BehaviorSignal::Terminated(Pid)` enum variant のみ。Pekko では `ref: ActorRef[Nothing]` を持つ独立 sealed class |
| `ChildFailed` 公開 signal 型 | `MessageAndSignals.scala:L104` | 部分実装 | core/typed | easy | 現在は `BehaviorSignal::ChildFailed { pid, error }` variant のみ。Pekko では `Terminated` のサブクラスで `cause: Throwable` を持つ独立型 |
| `MessageAdaptionFailure` signal | `MessageAndSignals.scala:L125` | 未対応 | core/typed | easy | メッセージアダプタ変換失敗時の公開 signal 型。`AdapterError` enum はあるがこれは内部エラー型であり、signal としての surface がない |
| `BehaviorBuilder` (Java DSL) | `javadsl/BehaviorBuilder.scala` | n/a | - | n/a | Java DSL 専用 builder |
| `ReceiveBuilder` (Java DSL) | `javadsl/ReceiveBuilder.scala` | n/a | - | n/a | Java DSL 専用 builder |
| `AbstractMatchingBehavior` (Java DSL) | `javadsl/AbstractMatchingBehavior.scala` | n/a | - | n/a | Java DSL 専用 |

実装済み型: `Behavior`, `Receive`, `Behaviors` (setup/receive/receiveMessage/withTimers/withStash/logMessages/withMdc/intercept/transformMessages/monitor/stopped), `TypedActorContext`, `TypedActorRef`, `TypedActorSystem`, `ActorRefResolver`, `AbstractBehavior` trait, `BehaviorInterceptor`, `BehaviorSignalInterceptor`, `BehaviorSignal`, `PreRestart`, `PostStop`, `DeathPactError`, `Signal` trait, `LogOptions`, `DispatcherSelector`, `MailboxSelector`, `TypedProps`, `ActorTags`, `SpawnProtocol`, `RecipientRef`, `MessageAdapterRegistry`, `AdapterPayload`, `TypedAskFuture`, `TypedAskResponse`, `StatusReply`, `FsmBuilder`, `BackoffSupervisorStrategy`, `RestartSupervisorStrategy`, `SupervisorStrategy` (typed)

### dispatch / mailbox ✅ 実装済み 11/13 (85%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ControlAwareMessageQueue` / `ControlMessage` | `Mailbox.scala:L871-902` | 未対応 | core/kernel | medium | 制御メッセージを通常メッセージより優先するキュー。`UnboundedControlAwareMailboxType` の追加が必要 |
| `NonBlockingBoundedMailbox` | `Mailbox.scala:L688` | 未対応 | core/kernel | easy | backpressure ベースの bounded mailbox。既存 `BoundedMessageQueue` のバリエーション |

実装済み型: `Mailbox`, `MessageQueue` trait, `MailboxType` trait, `Envelope`, `UnboundedMessageQueue`, `BoundedMessageQueue`, `UnboundedDequeMessageQueue`, `UnboundedPriorityMessageQueue`, `BoundedPriorityMessageQueue`, `UnboundedStablePriorityMessageQueue`, `BoundedStablePriorityMessageQueue`, `MessagePriorityGenerator` trait, `MailboxCapacity`, `MailboxOverflowStrategy`, `MailboxPolicy`, `Mailboxes`, `Dispatchers`, `DefaultDispatcher`, `PinnedDispatcher`, `BalancingDispatcher`, `Executor` trait, `InlineExecutor`, `MessageDispatcher` trait, `DispatcherSettings`, `SharedMessageQueue`

### event / logging ✅ 実装済み 8/9 (89%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `LoggingFilter` / `LoggingFilterWithMarker` | `Logging.scala:L1572-1604` | 未対応 | core/kernel | medium | ログレベルフィルタリング機構。現在の `LoggerWriter` trait / `LoggingAdapter` にフィルタ概念がない。Pekko ではロガー初期化時にフィルタをセットする仕組み |

実装済み型: `EventStream`, `EventStreamSubscriber` trait, `EventStreamSubscription`, `LogEvent`, `LogLevel`, `LoggingAdapter`, `BusLogging`, `NoLogging`, `ActorLogging`, `DiagnosticActorLogging`, `ActorLogMarker`, `LoggingReceive`, `LoggerSubscriber` (core), `TracingLoggerSubscriber` / `DeadLetterLogSubscriber` (std)

### pattern ✅ 実装済み 4/5 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `CircuitBreakersRegistry` (Extension) | `CircuitBreakersRegistry.scala:L35-65` | 未対応 | std | medium | `CircuitBreaker` 自体は core に実装済みだが、名前ベースで CB インスタンスを管理する Extension レジストリがない。`ActorSystem` の Extension 機構を活用して実装 |

実装済み型: `CircuitBreaker`, `CircuitBreakerShared`, `CircuitBreakerState`, `CircuitBreakerOpenError`, `CircuitBreakerCallError`, `Clock` trait, `ask_with_timeout`, `graceful_stop`, `graceful_stop_with_message`, `retry`

### classic routing ✅ 実装済み 6/9 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ConsistentHashingRoutingLogic` (classic kernel) | `ConsistentHashing.scala:L158` | 未対応 | core/kernel | medium | typed `PoolRouter::with_consistent_hash` は存在するが、kernel 層の独立した `RoutingLogic` 実装がない。`ConsistentHash` ユーティリティの core 実装が必要 |
| `SmallestMailboxRoutingLogic` (classic kernel) | `SmallestMailbox.scala:L48` | 未対応 | core/kernel | medium | typed `PoolRouter::with_smallest_mailbox` は存在するが、kernel 層の独立した `RoutingLogic` 実装がない。メールボックスサイズの取得 API が前提 |
| classic `Pool` / `Group` router config infrastructure | `RouterConfig.scala:L144-266` | 未対応 | core/kernel | hard | Pekko classic の `RouterConfig`, `Pool`, `Group`, `CustomRouterConfig`, `FromConfig` 等の設定駆動ルータ基盤。typed 側は `PoolRouter` / `GroupRouter` で代替済みだが、classic kernel の設定駆動ルーティング基盤は未実装 |

実装済み型 (kernel): `RoutingLogic` trait, `Router`, `Routee`, `Broadcast`, `RandomRoutingLogic`, `RoundRobinRoutingLogic`, `RouterCommand`, `RouterResponse`

### typed routing ✅ 実装済み 6/6 (100%)

ギャップなし。`Routers`, `PoolRouter`, `GroupRouter`, `BalancingPoolRouterBuilder`, `ScatterGatherFirstCompletedRouterBuilder`, `TailChoppingRouterBuilder`, `DefaultResizer`, `Resizer` trait は主要契約をカバー。

### discovery / receptionist ✅ 実装済み 9/9 (100%)

ギャップなし。`Receptionist`, `ServiceKey`, `Register`, `Deregister`, `Subscribe`, `Find`, `Listing`, `Registered`, `Deregistered` は主要契約をカバー。

### scheduling / timers ✅ 実装済み 8/8 (100%)

ギャップなし。classic `Scheduler` / `ClassicTimerScheduler` / `Cancellable`、typed `Scheduler` / `TimerScheduler` は実装済み。

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

### std adaptor ✅ 実装済み 4/6 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `VirtualThreadExecutorConfigurator` | `AbstractDispatcher.scala:L413` | 未対応 | std | easy | Java 21+ の仮想スレッド対応。Rust では tokio / smol がこの役割を果たすため、「仮想スレッド用 executor」は実質不要だが、Pekko parity としては `VirtualThreadExecutor` 相当の adapter が選択肢に入る |
| `AffinityPool` executor | `dispatch/affinity/AffinityPool.scala` | 未対応 | std | hard | CPU affinity ベースのスレッドプール。高度なスケジューリング最適化。Rust では tokio の work-stealing が同等の役割を担うが、独立した affinity executor は未実装 |

実装済み型 (std): `TokioExecutor`, `TokioExecutorFactory`, `PinnedExecutor`, `PinnedExecutorFactory`, `ThreadedExecutor`, `StdClock`, `StdBlocker`, `TracingLoggerSubscriber`, `DeadLetterLogSubscriber`

## 内部モジュール構造ギャップ

API ギャップが 85% まで詰まっており、主要カテゴリの致命的欠落は限定的なので、内部構造ギャップも分析対象に含める。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|----------------|--------|--------|------|
| receptionist の facade / protocol / runtime 実装がまだ粗く同居 | `actor-typed/receptionist/Receptionist.scala`, `actor-typed/internal/receptionist/ReceptionistMessages.scala` | `core/typed/receptionist.rs` が facade + behavior を保持し、protocol 型だけ `receptionist/` 配下に分割 | `core/typed/receptionist/` に behavior 実装も寄せ、公開 facade と内部実装の境界を明確化 | medium | high | 今後 serializer / cluster receptionist 拡張を入れると 1 ファイル集中が重くなる |
| typed delivery に `internal` 層がなく、公開型と制御ロジックが同じ階層に並ぶ | `actor-typed/delivery/*`, `actor-typed/delivery/internal/ProducerControllerImpl.scala` | `core/typed/delivery/` 直下に command / settings / behavior / state が並列 | `delivery/internal/` を新設し、controller 実装詳細と公開 DTO を分離 | medium | medium | 現時点で API は揃っているが、再送・永続キュー拡張時に責務が散りやすい |
| classic kernel の public surface が広く、内部補助型まで `pub` に露出しやすい | Pekko classic は package-private / internal API が多い | `core/kernel/**` に利用者向けでない `pub` 型が広く存在（343 pub 型は Pekko 比で多い） | `pub(crate)` へ寄せられるものを継続的に縮小し、入口 facade からの再公開を基準に露出制御 | medium | medium | fraktor は `pub` 露出が多く、型数だけで見ると Pekko を上回る |
| classic routing の kernel 層に ConsistentHash / SmallestMailbox RoutingLogic がない | Pekko `routing/ConsistentHashing.scala`, `routing/SmallestMailbox.scala` | typed `PoolRouter` 上のメソッドとしては存在するが、kernel `RoutingLogic` 実装がない | `core/kernel/routing/` に `ConsistentHashRoutingLogic` と `SmallestMailboxRoutingLogic` を追加 | medium | medium | typed 層から kernel 層へロジックを降ろす構造変更 |

## 実装優先度

### Phase 1（trivial / easy）

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| `ActorSystem.registerOnTermination` 相当 convenience | core/kernel | 既存 `TerminationSignal` に callback 登録メソッドを追加するだけ |
| `PoisonPill` / `Kill` の classic 公開型追加 | core/kernel | 内部 `SystemMessage` variant は存在するので公開 newtype を追加 |
| `ExtensibleBehavior` 相当の公開型 | core/typed | 既存 `Behavior` の薄い公開 trait / abstract base で吸収 |
| `Terminated` 公開 signal wrapper | core/typed | 既存 `BehaviorSignal::Terminated` を独立 struct に昇格 |
| `ChildFailed` 公開 signal wrapper | core/typed | 既存 `BehaviorSignal::ChildFailed` を独立 struct に昇格 |
| `MessageAdaptionFailure` signal | core/typed | 既存 `AdapterError` を signal として surface に追加 |
| `NonBlockingBoundedMailbox` | core/kernel | 既存 `BoundedMessageQueue` のバリエーション追加 |
| `VirtualThreadExecutorConfigurator` 相当 | std | Rust では tokio が仮想スレッド相当だが、設定名としての parity |

### Phase 2（medium）

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| `AllForOneStrategy` | core/kernel | 兄弟アクターの一括再起動制御ロジックの追加が必要 |
| `ControlAwareMessageQueue` / `ControlMessage` | core/kernel | 制御メッセージ優先キューの新規実装 |
| `ConsistentHashingRoutingLogic` (kernel) | core/kernel | kernel 層への独立 RoutingLogic 追加と ConsistentHash ユーティリティ |
| `SmallestMailboxRoutingLogic` (kernel) | core/kernel | kernel 層への独立 RoutingLogic 追加とメールボックスサイズ取得 API |
| `LoggingFilter` / `LoggingFilterWithMarker` | core/kernel | ロガー初期化にフィルタ概念を追加 |
| `CircuitBreakersRegistry` | std | Extension 機構を活用した名前ベース CB レジストリ |
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
| `VirtualizedExecutorService` | Java 21 仮想スレッド固有。Rust では tokio が同等 |

## まとめ

- actor モジュールの parity は **主要 typed 契約がかなり埋まっている（85%）**。特に typed routing / receptionist / delivery / pubsub / ref resolver / serialization / extension / coordinated shutdown は完全カバー。
- 低コストで前進できるのは:
  - `registerOnTermination` convenience、`PoisonPill`/`Kill` 公開型（Phase 1 kernel）
  - `ExtensibleBehavior`、`Terminated`/`ChildFailed` 独立 signal 型、`MessageAdaptionFailure`（Phase 1 typed）
- parity 上の主要ギャップは:
  - `AllForOneStrategy`（Phase 2）: 兄弟アクター一括再起動制御
  - classic routing の kernel 層 RoutingLogic 不足（`ConsistentHash`/`SmallestMailbox`）（Phase 2）
  - classic `RouterConfig` 設定駆動基盤（Phase 3）
- 次のボトルネックは API 不足そのものよりも、**receptionist / delivery の内部責務の切り方**、および **kernel 層の `pub` 露出過多** に移りつつある。API gap はまだ残るが、構造整理を並行して進めないと以後の parity 実装速度が落ちる。
