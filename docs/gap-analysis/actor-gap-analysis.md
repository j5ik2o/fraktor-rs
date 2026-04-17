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
- 分析日: 2026-04-17（初版: 2026-04-15、第2版: 2026-04-16、第3版: 2026-04-17、第4版: 2026-04-17）
- 第3版での追加検出: Pekko 側を `actor` / `actor-typed` 両パッケージから全件再抽出し、ergonomics 系 API と classic 補助パターンの未対応項目を新たに洗い出した。
- 第4版での更新: `SmallestMailboxRoutingLogic` の Pekko 互換化を実装完了（2パス探索・`isSuspended`/`isProcessingMessage` 追跡・スコアリング）。部分実装ギャップは 1 件に減少。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（parity 対象） | 102 |
| fraktor-rs 対応実装数 | 93（うち完全実装 92 / 部分実装 1） |
| カバレッジ（型単位） | 93/102 ≈ 91% |
| ギャップ数 | 10（未対応 9 + 部分実装 1、core/kernel: 7, core/typed: 2, std: 1） |
| 部分実装ギャップ | 1（kernel 側 `ConsistentHashingRoutingLogic`。型は存在するが Pekko 互換の挙動が欠落。**本項目は「実装数 93」と「ギャップ数 10」の両方に計上される**ため `93 + 10 ≠ 102` になる点に注意。未対応分は 102 − 93 = 9） |
| n/a 除外数 | 約 60（Java DSL, IO, japi, internal, JVM 固有） |

enumerated gaps (カテゴリ別ギャップから再掲):
- **core/kernel (7)**: `LoggingFilter` / `LoggingFilterWithMarker`、classic `Pool` / `Group` RouterConfig 基盤、`ConsistentHashableEnvelope`、`ConsistentHash<T>` / `MurmurHash` util、`OptimalSizeExploringResizer`、`Listeners` / `Listen` / `Deafen` / `WithListeners`、`ConsistentHashingRoutingLogic`（partial）
- **core/typed (2)**: `LoggerOps`、typed `OptimalSizeExploringResizer` expose
- **std (1)**: `AffinityPool` executor

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 40 | 34 | 34/40 = 85% |
| core / typed ラッパー | 56 | 54 | 54/56 ≈ 96% |
| std / アダプタ | 6 | 5 | 5/6 ≈ 83% |
| 合計 | 102 | 93 | 93/102 ≈ 91% |

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

### dispatch / mailbox ✅ 実装済み 13/13 (100%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| ~~`NonBlockingBoundedMailbox`~~ | ~~`Mailbox.scala:L688`~~ | ~~未対応~~ | ~~core/kernel~~ | ~~easy~~ | **候補から除外**: `BoundedMessageQueue::new(cap, MailboxOverflowStrategy::DropNewest)`（`bounded_message_queue.rs:26`）が意味的に等価（enqueue は非ブロッキングかつ overflow 時に即破棄）。Pekko の独立型は Lock-free MPSC という実装選択のための別名であり、fraktor-rs では overflow strategy の選択肢として統合済み。名前だけの parity のため追加実装は不要。 |

実装済み型: `Mailbox`, `MessageQueue` trait, `MailboxType` trait, `Envelope`, `UnboundedMessageQueue`, `BoundedMessageQueue`, `UnboundedDequeMessageQueue`, `UnboundedPriorityMessageQueue`, `BoundedPriorityMessageQueue`, `UnboundedStablePriorityMessageQueue`, `BoundedStablePriorityMessageQueue`, `UnboundedControlAwareMessageQueue`, `UnboundedControlAwareMailboxType`, `MessagePriorityGenerator` trait, `MailboxCapacity`, `MailboxOverflowStrategy`, `MailboxPolicy`, `Mailboxes`, `Dispatchers`, `DefaultDispatcher`, `PinnedDispatcher`, `BalancingDispatcher`, `Executor` trait, `InlineExecutor`, `MessageDispatcher` trait, `DispatcherSettings`, `SharedMessageQueue`

### event / logging ✅ 実装済み 8/10 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `LoggingFilter` / `LoggingFilterWithMarker` | `Logging.scala:L1572-1604` | 未対応 | core/kernel | medium | ログレベルフィルタリング機構。現在の `LoggerWriter` trait / `LoggingAdapter` にフィルタ概念がない |
| `LoggerOps` (N-arg / 2-arg log helpers) | `actor-typed/scaladsl/package.scala:L1-105` | 未対応 | core/typed | easy | `trace2/debug2/info2/warn2/error2` と `traceN/debugN/infoN/warnN/errorN` の可変引数ロガー拡張。Pekko の Scala DSL ergonomics の中核。現 `TypedActorSystemLog::emit` は単一メッセージのみ |

実装済み型: `EventStream`, `EventStreamSubscriber` trait, `EventStreamSubscription`, `LogEvent`, `LogLevel`, `LoggingAdapter`, `BusLogging`, `NoLogging`, `ActorLogging`, `DiagnosticActorLogging`, `ActorLogMarker`, `LoggingReceive`, `LoggerSubscriber` (core), `TracingLoggerSubscriber` / `DeadLetterLogSubscriber` (std)

備考: Pekko の `EventBus` trait（EventStream とは別の汎用イベントバス抽象）は未実装だが、fraktor では `EventStreamSubscriber` trait が同等の役割を果たしており、実質的な機能欠落はない。独立した汎用 `EventBus` trait の必要性は低い。`Logging.Error/Warning/Info/Debug` 独立 case class は fraktor の `LogEvent` 列挙型で機能的にカバー済みのため parity 対象外。

### pattern ✅ 実装済み 5/5 (100%)

ギャップなし。前回分析時に未対応としていた `CircuitBreakersRegistry` が `modules/actor-adaptor-std/src/std/pattern/circuit_breakers_registry.rs` に実装済みであることを確認。`Extension` trait を実装し、`from_actor_system` / `get` / `with_named_config` 等のメソッドで名前ベースの CB インスタンス管理を提供。

実装済み型: `CircuitBreaker`, `CircuitBreakerShared`, `CircuitBreakerState`, `CircuitBreakerOpenError`, `CircuitBreakerCallError`, `Clock` trait, `CircuitBreakersRegistry`, `ask_with_timeout`, `graceful_stop`, `graceful_stop_with_message`, `retry`, `pipe_to` / `pipe_to_self` (ActorContext メソッド)

### classic routing ✅ 実装済み 10/16 (63%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ConsistentHashingRoutingLogic` (classic kernel) | `ConsistentHashing.scala:L158` | 部分実装 | core/kernel | medium | kernel に独立 `RoutingLogic` 実装 (`consistent_hashing_routing_logic.rs`) が存在することを確認。一方で Pekko の `ConsistentHashableEnvelope`、`hashMapping` 抽象、virtual nodes 調整ロジックは未対応 |
| classic `Pool` / `Group` router config infrastructure | `RouterConfig.scala:L144-266` | 未対応 | core/kernel | hard | Pekko classic の `RouterConfig`, `Pool`, `Group`, `CustomRouterConfig`, `FromConfig` 等の設定駆動ルータ基盤。typed 側は `PoolRouter` / `GroupRouter` で代替済み |
| `ConsistentHashableEnvelope` | `ConsistentHashing.scala:L67` | 未対応 | core/kernel | easy | 一貫性ハッシュ用メッセージラッパー。kernel ルーティングと一体で導入するとよい |
| `ConsistentHash<T>` / `MurmurHash` | `ConsistentHash.scala`, `MurmurHash.scala` | 未対応 | core/kernel (util) | medium | 一貫性ハッシュリング実装。kernel `ConsistentHashingRoutingLogic` の完全化と一体で要対応。現 fraktor-rs 実装は hash に依存しているが独立公開 util が無い |
| `OptimalSizeExploringResizer` | `OptimalSizeExploringResizer.scala:L31` | 未対応 | core/kernel | hard | 最適サイズ探索リサイザー。現状は `DefaultResizer` のみ。大規模プールの自動最適化に必要 |
| `Listeners` trait / `Listen` / `Deafen` / `WithListeners` | `routing/Listeners.scala:L20-36` | 未対応 | core/kernel | easy | リスナー管理ミックスイン。EventStream 購読で代替可能だが、アクター内部に listener 集合を持つ classic パターンが欲しい場合は要実装 |

実装済み型 (kernel): `RoutingLogic` trait, `Router`, `Routee`, `Broadcast`, `RandomRoutingLogic`, `RoundRobinRoutingLogic`, `ConsistentHashingRoutingLogic`（簡略版）, `SmallestMailboxRoutingLogic`（Pekko 互換完全実装: 2パス探索・`isSuspended`/`isProcessingMessage` 追跡・スコアリング）, `RouterCommand`, `RouterResponse`

備考: `Router::addRoutee` / `removeRoutee` 相当の動的ルーティー管理 API は Pekko 固有。fraktor-rs は静的ルーティー前提で設計され、resizer により数量だけを動的に調整する方針のため parity 対象外。

### typed routing ✅ 実装済み 6/7 (86%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| typed `OptimalSizeExploringResizer` 相当 | (Pekko classic にのみ存在、typed からは `Resizer` として利用可) | 未対応 | core/typed | hard | classic 側の `OptimalSizeExploringResizer` 実装後、typed `PoolRouter::with_resizer` から利用可能にする必要がある |

実装済み型: `Routers`, `PoolRouter`, `GroupRouter`, `BalancingPoolRouterBuilder`, `ScatterGatherFirstCompletedRouterBuilder`, `TailChoppingRouterBuilder`, `DefaultResizer`, `Resizer` trait。ConsistentHash / SmallestMailbox は `PoolRouter` / `GroupRouter` のメソッドとして利用可能。

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
| classic routing の kernel 層 `ConsistentHashingRoutingLogic` が簡略実装 | Pekko `routing/ConsistentHashing.scala` | kernel に `consistent_hashing_routing_logic.rs` が存在するが、`hashMapping` 抽象 / virtual nodes 調整 / `ConsistentHashableEnvelope` が未対応 | `core/kernel/routing/` の `ConsistentHashingRoutingLogic` を Pekko 互換化し、`ConsistentHash<T>` / `MurmurHash` 公開 util を追加 | medium | medium | `SmallestMailboxRoutingLogic` は第4版で Pekko 互換化完了済み（2 パス探索 / `isSuspended` / `isProcessingMessage` 追跡）。残る classic routing 側 kernel ギャップは ConsistentHash のみ |

## 実装優先度

### Phase 1（trivial / easy）

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| `LoggerOps` 相当の N-arg / 2-arg log helpers | core/typed | typed DSL ergonomics。既存 `TypedActorSystemLog::emit` の上に build 可能。Pekko の `trace2/debug2/info2/warn2/error2` + `traceN/...` と同等 API を提供する |
| `ConsistentHashableEnvelope` | core/kernel | 一貫性ハッシュ用メッセージラッパー。既存 kernel `ConsistentHashingRoutingLogic` の hashMapping 導入と一体で追加 |

### Phase 2（medium）

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| `ConsistentHashingRoutingLogic` 完全化 (`hashMapping` 抽象, virtual nodes 調整) | core/kernel | 現状は簡略版。`ConsistentHash<T>` / `MurmurHash` 公開 util を新設し、`hashMapping` と virtual nodes 対応を追加 |
| `ConsistentHash<T>` / `MurmurHash` util 公開 | core/kernel (util) | 上記 ConsistentHash 完全化と一体。classic 互換の virtual-node ベースリングを公開 |
| `LoggingFilter` / `LoggingFilterWithMarker` | core/kernel | ロガー初期化にフィルタ概念を追加。`LoggerWriter` trait を拡張 |
| `Listeners` trait / `Listen` / `Deafen` / `WithListeners` | core/kernel | リスナー管理ミックスイン。actor 内部の購読者集合パターンが欲しい用途のため |
| receptionist 実装の `receptionist/` 配下への再配置 | core/typed | API を壊さず責務を整理できるが、ファイル分割は複数箇所に波及する |
| delivery の `internal` 分離 | core/typed | 既存 controller 群の責務整理が必要 |

### Phase 3（hard）

| 項目 | 実装先層 | 理由 |
|------|----------|------|
| classic `Pool` / `Group` router config infrastructure | core/kernel | 設定駆動ルーティング基盤の新規設計。`RouterConfig`, `Pool`, `Group`, `CustomRouterConfig`, `FromConfig` 相当 |
| `OptimalSizeExploringResizer` (classic + typed expose) | core/kernel + core/typed | 最適サイズ探索リサイザーの新規設計。遅延測定・帯域測定・自動増減アルゴリズムを含む |
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

- actor モジュールの parity は **約 91%**（93/102 型）に達しており、主要機能は概ねカバー済み。第3版での Pekko 両パッケージ全件再抽出により、第2版では未検出だった ergonomics 系 API（`LoggerOps`）および classic 補助パターン（`Listeners`, `OptimalSizeExploringResizer`, `ConsistentHash` ユーティリティ等）の欠落を新たに検出した。第4版で `SmallestMailboxRoutingLogic` の Pekko 互換化が完了し、残る部分実装ギャップは `ConsistentHashingRoutingLogic` の 1 件のみ。
- **完全カバー済みカテゴリ**（100%）: classic actor core, supervision, typed core surface, receptionist, scheduling/timers, ref/resolution, delivery/pubsub, serialization, extension, coordinated shutdown, pattern, dispatch/mailbox — **12カテゴリ**で完全 parity。
- enumerated gap 計 10 件（kernel 7, typed 2, std 1、うち partial 1）:
  - parity を低コストで前進できる未対応機能（Phase 1 = trivial/easy）:
    - `LoggerOps` 相当の N-arg / 2-arg ロガー拡張（core/typed, easy）
    - `ConsistentHashableEnvelope`（core/kernel, easy）
  - parity 上の主要ギャップ（Phase 2-3）:
    - `ConsistentHashingRoutingLogic` の Pekko 互換化（partial、`ConsistentHash<T>` / `MurmurHash` 公開 util が必要, Phase 2 medium）
    - `LoggingFilter` / `LoggingFilterWithMarker`（core/kernel, Phase 2 medium）
    - `Listeners` / `Listen` / `Deafen`（core/kernel, Phase 2 easy〜medium）
    - `OptimalSizeExploringResizer`（core/kernel + typed expose, Phase 3 hard）
    - classic `Pool` / `Group` RouterConfig 基盤（core/kernel, Phase 3 hard）
    - `AffinityPool` executor（std, Phase 3 hard、tokio の work-stealing で代替可）
- 次のボトルネックは API 不足そのものよりも、**kernel 側 `ConsistentHashingRoutingLogic` の Pekko 互換挙動差分**（virtual nodes 調整と `hashMapping` 抽象）と、**receptionist / delivery の内部責務の切り方**、および **kernel 層の `pub` 露出過多** にある。Phase 2 の medium 項目を順次埋めつつ、構造整理を並行することで、以後の parity 実装速度を維持できる。
