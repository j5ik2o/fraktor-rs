# actor モジュール ギャップ分析

## 前提と集計範囲

- 比較対象:
  - fraktor-rs: `modules/actor/src/core`, `modules/actor/src/std`
  - Pekko: `references/pekko/actor/src/main/scala/org/apache/pekko/actor`, `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed`
- `typed` 層は `references/pekko/actor-typed` を対応先に含める。`modules/actor/src/core/typed` を無視すると typed parity が過小評価になるため。
- 集計対象は parity に直接効く公開型・主要 DSL・主要 runtime surface に限定する。
- 除外対象:
  - `internal` パッケージ
  - `javadsl` / `japi` の別名 API
  - `testkit`
  - `boilerplate`
  - `util`
- 型数はオーバーロードを 1 件に集約した概数。カテゴリ見出しの `X/Y` は「主要公開 surface の対応数」であり、ギャップ表は未対応・部分実装・`n/a` のみを列挙する。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 約 46 |
| fraktor-rs 公開型数 | 約 33（core: 28, std: 5） |
| カバレッジ（型単位） | 約 33/46 (72%) |
| ギャップ数 | 13（core: 10, std: 2, n/a: 1） |

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 16 | 11 | 69% |
| core / typed ラッパー | 22 | 17 | 77% |
| std / アダプタ | 8 | 5 | 63% |

## カテゴリ別ギャップ

### classic / runtime 基盤　✅ 実装済み 10/15 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `BackoffSupervisor` | `references/pekko/actor/src/main/scala/org/apache/pekko/pattern/BackoffSupervisor.scala:L22` | 未対応 | core/kernel | hard | fraktor-rs には `BackoffSupervisorStrategy` (`modules/actor/src/core/kernel/actor/supervision/backoff_supervisor_strategy.rs:L18`) はあるが、専用 supervisor actor / props / protocol surface はない |
| `Router` / `RouterConfig` | `references/pekko/actor/src/main/scala/org/apache/pekko/routing/Router.scala:L110`, `references/pekko/actor/src/main/scala/org/apache/pekko/routing/RouterConfig.scala:L52` | 未対応 | core/kernel | hard | fraktor-rs は typed router builder (`modules/actor/src/core/typed/dsl/routing/routers.rs:L17`) を持つが、classic router object/config API は未実装 |
| `PipeToSupport` | `references/pekko/actor/src/main/scala/org/apache/pekko/pattern/PipeToSupport.scala:L28` | 部分実装 | core/kernel | medium | `pipe_to_self` は classic/typed の context にある (`modules/actor/src/core/kernel/actor/actor_context.rs:L392`, `modules/actor/src/core/typed/actor/actor_context.rs:L391`) が、外部 actor / completion-stage へ流す `pipeTo` surface はない |
| `Tcp` / `Udp` / `Dns` extension family | `references/pekko/actor/src/main/scala/org/apache/pekko/io/Tcp.scala:L50`, `references/pekko/actor/src/main/scala/org/apache/pekko/io/Udp.scala:L42`, `references/pekko/actor/src/main/scala/org/apache/pekko/io/Dns.scala:L49` | 未対応 | std | hard | `modules/actor/src` には `io/` 相当の公開 surface が存在しない。actor モジュールは scheduler / dispatch / event まではあるが、socket/DNS API は未着手 |
| `DynamicAccess` / `ReflectiveDynamicAccess` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/DynamicAccess.scala:L33`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ReflectiveDynamicAccess.scala:L35` | 未対応 | n/a | n/a | JVM の classloader / reflection 前提 API。Rust/no_std 制約の下では同名 parity をそのまま実装する必然性が薄い |

### typed system / extension　✅ 実装済み 7/12 (58%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Dispatchers` facade | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Dispatchers.scala:L37` | 部分実装 | core/typed | medium | `modules/actor/src/core/typed/dispatchers.rs:L1` は placeholder で、公開 `Dispatchers` 型がまだない |
| `Scheduler` facade | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Scheduler.scala:L31`, `:L75`, `:L136` | 部分実装 | core/typed | medium | `TypedActorSystem::scheduler` は `TypedSchedulerShared` を返す (`modules/actor/src/core/typed/system.rs:L185`, `modules/actor/src/core/typed/internal/typed_scheduler_shared.rs:L7`) が、返り値自身には Pekko 相当の direct scheduling surface がない |
| `ActorSystem` metadata/accessors (`name`, `settings`, `logConfiguration`, `log`, `startTime`, `uptime`, `dispatchers`, `getWhenTerminated`) | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:L52`, `:L57`, `:L62`, `:L70`, `:L75`, `:L80`, `:L106`, `:L148` | 未対応 | core/typed | medium | fraktor-rs の `TypedActorSystem` は lifecycle / receptionist / scheduler / extension を公開している (`modules/actor/src/core/typed/system.rs:L34`) が、system metadata 系 accessor は揃っていない |
| `AbstractExtensionSetup` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Extensions.scala:L186` | 部分実装 | core/typed | easy | 汎用 `ExtensionSetup` はある (`modules/actor/src/core/typed/extension_setup.rs:L16`) が、Pekko の abstract base 相当は未提供 |
| `ActorContext.setLoggerName` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/ActorContext.scala:L99`, `:L108` | 部分実装 | std | easy | logger 名指定は `LogOptions::with_logger_name` (`modules/actor/src/core/typed/log_options.rs:L41`) で可能だが、context mutation API としては露出していない |

### typed DSL / supervision　✅ 実装済み 7/9 (78%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Receive` builder | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Behaviors.scala:L330` | 未対応 | core/typed | medium | fraktor-rs は `Behaviors` (`modules/actor/src/core/typed/dsl/behaviors.rs:L93`) と `AbstractBehavior` (`modules/actor/src/core/typed/dsl/abstract_behavior.rs:L21`) はあるが、fluent `Receive` surface は持たない |
| `BackoffSupervisorStrategy.withCriticalLogLevelAfter` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/SupervisorStrategy.scala:L350` | 未対応 | core/kernel | easy | fraktor-rs は `critical_log_level_after` フィールドを保持している (`modules/actor/src/core/kernel/actor/supervision/backoff_supervisor_strategy.rs:L18`) が、builder method は `with_critical_log_level` までで止まっている |

### typed discovery / delivery　✅ 実装済み 9/10 (90%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `DurableProducerQueue` parity | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/delivery/DurableProducerQueue.scala:L33`, `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/delivery/ProducerController.scala:L257`, `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/delivery/WorkPullingProducerController.scala:L222` | 未対応 | core/typed | hard | fraktor-rs は `ProducerController` / `WorkPullingProducerController` 自体はある (`modules/actor/src/core/typed/delivery/producer_controller.rs:L96`, `modules/actor/src/core/typed/delivery/work_pulling_producer_controller.rs:L150`) が、durable queue と結びつく公開 API はない。`behavior_with_settings` も公開されていない |

## 内部モジュール構造ギャップ

今回は API ギャップが支配的なため省略する。

判定理由:

- `hard` / `medium` の未実装ギャップが 8 件あり、しきい値 5 件を超えている
- `Dispatchers` が placeholder のままで、typed runtime facade が未完成
- classic parity でも `BackoffSupervisor` / `RouterConfig` / `Tcp/Udp/Dns` が未実装

したがって、次フェーズは内部責務の切り方ではなく、公開契約 parity の穴埋めが優先である。

## 実装優先度

### Phase 1

- `AbstractExtensionSetup` を `ExtensionSetup` の薄い別名/基底として追加する（core/typed）
- `ActorContext.setLoggerName` を `LogOptions` 連携の sugar として追加する（std）
- `BackoffSupervisorStrategy.withCriticalLogLevelAfter` を追加する（core/kernel）

### Phase 2

- `Dispatchers` facade を placeholder から公開型へ引き上げる（core/typed）
- `Scheduler` facade を `TypedSchedulerShared` の内部 API ではなく public API として整える（core/typed）
- `TypedActorSystem` に system metadata/accessor surface を追加する（core/typed）
- `Receive` builder を `Behaviors` / `AbstractBehavior` の隣に追加する（core/typed）
- classic `PipeToSupport` 相当の外部 actor 連携 API を追加する（core/kernel）

### Phase 3

- `BackoffSupervisor` actor / protocol / props surface を実装する（core/kernel）
- classic `Router` / `RouterConfig` surface を実装する（core/kernel）
- `DurableProducerQueue` と `ProducerController` / `WorkPullingProducerController` の接続面を実装する（core/typed）
- `Tcp` / `Udp` / `Dns` extension family を実装する（std）

### 対象外（n/a）

- `DynamicAccess` / `ReflectiveDynamicAccess`（n/a）
  - JVM reflection / classloader 契約であり、Rust/no_std parity の直接対象にしない

## まとめ

- 全体カバレッジは約 72% で、typed の主要経路と classic の基本型はかなり揃っている。一方で、runtime facade と classic extras の不足が目立つ。
- 低コストで parity を前進できるのは `AbstractExtensionSetup`、`ActorContext.setLoggerName`、`BackoffSupervisorStrategy.withCriticalLogLevelAfter` の 3 点。
- 主要ギャップは `BackoffSupervisor`、classic `Router/RouterConfig`、`DurableProducerQueue`、`Tcp/Udp/Dns` で、いずれも新しい基盤 surface を伴う。
- 現時点では API ギャップが支配的であり、次のボトルネックは内部構造ではなく公開契約 parity にある。

## 完全候補一覧

この節は YAGNI を適用しない。

- 目的:
  - 「今は未優先だが後から芋づる式に出てくる」候補を先に全部見える化する
  - 主要 parity matrix に入れなかった helper type / language-specific DSL / internal-adjacent helper も落とさない
- 集計ルール:
  - package-qualified な top-level 型を広めに拾っている
  - `internal` / `dungeon` / `javadsl` のような補助 package も、後から出てくる候補を潰すためあえて残している
  - したがって、この節の件数は上の「主要 parity surface」の件数より多い

### 広義候補件数

| 区分 | package-qualified 候補数 |
|------|--------------------------|
| actor-typed | 185 |
| classic actor 系 | 561 |

### actor-typed 完全候補

- `typed::ActorRef.scala (2)`: ActorRef, RecipientRef
- `typed::ActorRefResolver.scala (2)`: ActorRefResolver, ActorRefResolverSetup
- `typed::ActorSystem.scala (2)`: ActorSystem, Settings
- `typed::Behavior.scala (3)`: Behavior, SuperviseBehavior, ExtensibleBehavior
- `typed::BehaviorInterceptor.scala (5)`: BehaviorInterceptor, PreStartTarget, ReceiveTarget, SignalTarget, BehaviorSignalInterceptor
- `typed::Dispatchers.scala (1)`: Dispatchers
- `typed::Extensions.scala (5)`: Extension, ExtensionId, Extensions, ExtensionSetup, AbstractExtensionSetup
- `typed::LogOptions.scala (1)`: LogOptions
- `typed::MessageAndSignals.scala (7)`: DeathPactException, Signal, PreRestart, PostStop, Terminated, ChildFailed, MessageAdaptionFailure
- `typed::Props.scala (4)`: Props, DispatcherSelector, MailboxSelector, ActorTags
- `typed::Scheduler.scala (1)`: Scheduler
- `typed::SpawnProtocol.scala (2)`: SpawnProtocol, Spawn
- `typed::SupervisorStrategy.scala (3)`: SupervisorStrategy, RestartSupervisorStrategy, BackoffSupervisorStrategy
- `typed::TypedActorContext.scala (1)`: TypedActorContext
- `typed::delivery (32)`: ConsumerController, Command, Start, Delivery, Confirmed, RegisterToProducerController, DeliverThenStop, SequencedMessage, Settings, DurableProducerQueue, Command, LoadState, StoreMessageSent, StoreMessageSentAck, StoreMessageConfirmed, State, MessageSent, ProducerController, Command, Start, RequestNext, MessageWithConfirmation, RegisterConsumer, Settings, WorkPullingProducerController, Command, Start, RequestNext, MessageWithConfirmation, GetWorkerStats, WorkerStats, Settings
- `typed::eventstream (4)`: EventStream, Publish, Subscribe, Unsubscribe
- `typed::internal (20)`: LoggingContext, ActorFlightRecorder, UnhandledBehavior, SameBehavior, FailedBehavior, DeferredBehavior, ReceiveBehavior, ReceiveMessageBehavior, EmptyProps, DispatcherDefault, DispatcherFromConfig, DispatcherSameAsParent, DefaultMailboxSelector, BoundedMailboxSelector, MailboxFromConfigSelector, ActorTagsImpl, ScheduledRestart, ResetRestartCount, Timer, TimerMsg
- `typed::internal/adapter (4)`: TypedActorFailedException, AdapterExtension, LoadTypedExtensions, Start
- `typed::internal/jfr (21)`: DeliveryProducerCreated, DeliveryProducerStarted, DeliveryProducerRequestNext, DeliveryProducerSent, DeliveryProducerWaitingForRequest, DeliveryProducerResentUnconfirmed, DeliveryProducerResentFirst, DeliveryProducerResentFirstUnconfirmed, DeliveryProducerReceived, DeliveryProducerReceivedRequest, DeliveryProducerReceivedResend, DeliveryConsumerCreated, DeliveryConsumerStarted, DeliveryConsumerReceived, DeliveryConsumerReceivedPreviousInProgress, DeliveryConsumerDuplicate, DeliveryConsumerMissing, DeliveryConsumerReceivedResend, DeliveryConsumerSentRequest, DeliveryConsumerChangedProducer, DeliveryConsumerStashFull
- `typed::internal/pubsub (9)`: Command, Publish, Subscribe, Unsubscribe, GetTopicStats, TopicStats, TopicInstancesUpdated, MessagePublished, SubscriberTerminated
- `typed::internal/receptionist (9)`: Register, Deregister, Registered, Deregistered, Find, Listing, Subscribe, DefaultServiceKey, ServiceKeySerializer
- `typed::internal/routing (3)`: RoundRobinLogic, RandomLogic, ConsistentHashingLogic
- `typed::javadsl (15)`: AbstractBehavior, AbstractMatchingBehavior, ActorContext, Adapter, AskPattern, BehaviorBuilder, Behaviors, Supervise, Receive, ReceiveBuilder, Routers, GroupRouter, PoolRouter, StashOverflowException, TimerScheduler
- `typed::pubsub (7)`: Topic, Command, Publish, Subscribe, Unsubscribe, TopicStats, GetTopicStats
- `typed::receptionist (10)`: Receptionist, ServiceKey, Listing, Registered, Register, Deregister, Deregistered, Subscribe, Find, ReceptionistSetup
- `typed::scaladsl (11)`: AbstractBehavior, ActorContext, AskPattern, Behaviors, Supervise, Receive, Routers, GroupRouter, PoolRouter, StashOverflowException, TimerScheduler
- `typed::scaladsl/adapter (1)`: PropsAdapter

### classic actor 系 完全候補

- `classic::actor (142)`: AbstractActor, Receive, ActorContext, UntypedAbstractActor, AbstractLoggingActor, UntypedAbstractLoggingActor, AbstractActorWithStash, UntypedAbstractActorWithStash, AbstractActorWithUnboundedStash, UntypedAbstractActorWithUnboundedStash, AbstractActorWithUnrestrictedStash, UntypedAbstractActorWithUnrestrictedStash, AbstractFSM, AbstractLoggingFSM, AbstractFSMWithStash, PossiblyHarmful, NoSerializationVerificationNeeded, PoisonPill, Kill, Identify, ActorIdentity, Terminated, ReceiveTimeout, NotInfluenceReceiveTimeout, IllegalActorStateException, ActorKilledException, InvalidActorNameException, ActorInitializationException, PreRestartException, PostRestartException, OriginalRestartException, InvalidMessageException, DeathPactException, ActorInterruptedException, UnhandledMessage, Status, Success, Failure, ActorLogging, DiagnosticActorLogging, Actor, ActorContext, ActorLogMarker, ActorPaths, ActorPath, RootActorPath, ChildActorPath, ActorRef, AllDeadLetters, DeadLetter, DeadLetterSuppression, SuppressedDeadLetter, Dropped, WrappedMessage, SerializedDeadLetterActorRef, ActorRefFactory, RegisterTerminationHook, TerminationHook, TerminationHookDone, ActorSelection, ScalaActorSelection, ActorNotFound, BootstrapSetup, ProviderSelection, Local, Remote, Cluster, Custom, ActorSystem, Settings, ExtendedActorSystem, TerminationCallbacks, Address, RelativeActorPath, AddressFromURIString, ActorPathExtractor, ClassicActorSystemProvider, ClassicActorContextProvider, CoordinatedShutdown, Reason, UnknownReason, ActorSystemTerminateReason, JvmExitReason, ClusterDowningReason, ClusterJoinUnsuccessfulReason, IncompatibleConfigurationDetectedReason, ClusterLeavingReason, Watch, Deploy, Scope, LocalScope, NoScopeGiven, Extension, ExtensionId, AbstractExtensionId, ExtensionIdProvider, FSM, NullFunction, CurrentState, Transition, SubscribeTransitionCallBack, UnsubscribeTransitionCallBack, Reason, Normal, Shutdown, Failure, StateTimeout, LogEntry, State, Event, StopEvent, TransformHelper, LoggingFSM, ChildRestartStats, SupervisorStrategyConfigurator, DefaultSupervisorStrategy, StoppingSupervisorStrategy, SupervisorStrategyLowPriorityImplicits, SupervisorStrategy, Directive, Resume, Restart, Stop, Escalate, AllForOneStrategy, OneForOneStrategy, IndirectActorProducer, LightArrayRevolverScheduler, Props, ReflectiveDynamicAccess, Scheduler, AbstractSchedulerBase, SchedulerTask, Cancellable, TaskRunOnClose, Stash, UnboundedStash, UnrestrictedStash, StashOverflowException, Timers, AbstractActorWithTimers, UntypedAbstractActorWithTimers
- `classic::actor/dungeon (18)`: SuspendReason, UserRequest, Recreation, Creation, Termination, ChildRestartsIterable, ChildrenIterable, WaitingForChildren, EmptyChildrenContainer, TerminatedChildrenContainer, NormalChildrenContainer, TerminatingChildrenContainer, SerializationCheckFailedException, FailedInfo, TimerMsg, Timer, InfluenceReceiveTimeoutTimerMsg, NotInfluenceReceiveTimeoutTimerMsg
- `classic::actor/setup (2)`: Setup, ActorSystemSetup
- `classic::dispatch (73)`: Envelope, TaskInvocation, MessageDispatcher, ExecutorServiceConfigurator, MessageDispatcherConfigurator, VirtualThreadExecutorConfigurator, ThreadPoolExecutorServiceFactoryProvider, ThreadPoolExecutorServiceFactory, ThreadPoolExecutorConfigurator, DefaultExecutorServiceConfigurator, PathEntry, ValuePathEntry, StringPathEntry, CompletionStages, Dispatcher, PriorityGenerator, DispatcherPrerequisites, Dispatchers, DispatcherConfigurator, BalancingDispatcherConfigurator, PinnedDispatcherConfigurator, ForkJoinExecutorConfigurator, PekkoForkJoinPool, PekkoForkJoinTask, ForkJoinExecutorServiceFactory, ExecutionContexts, Futures, MessageQueue, NodeMessageQueue, BoundedNodeMessageQueue, MultipleConsumerSemantics, QueueBasedMessageQueue, UnboundedMessageQueueSemantics, UnboundedQueueBasedMessageQueue, BoundedMessageQueueSemantics, BoundedQueueBasedMessageQueue, DequeBasedMessageQueueSemantics, UnboundedDequeBasedMessageQueueSemantics, BoundedDequeBasedMessageQueueSemantics, DequeBasedMessageQueue, UnboundedDequeBasedMessageQueue, BoundedDequeBasedMessageQueue, MailboxType, ProducesMessageQueue, UnboundedMailbox, SingleConsumerOnlyUnboundedMailbox, NonBlockingBoundedMailbox, BoundedMailbox, UnboundedPriorityMailbox, BoundedPriorityMailbox, UnboundedStablePriorityMailbox, BoundedStablePriorityMailbox, UnboundedDequeBasedMailbox, BoundedDequeBasedMailbox, ControlAwareMessageQueueSemantics, UnboundedControlAwareMessageQueueSemantics, BoundedControlAwareMessageQueueSemantics, ControlMessage, UnboundedControlAwareMailbox, BoundedControlAwareMailbox, RequiresMessageQueue, Mailboxes, PinnedDispatcher, ThreadPoolConfig, ExecutorServiceFactory, ExecutorServiceFactoryProvider, ThreadPoolConfigBuilder, MonitorableThreadFactory, MonitorableCarrierThreadFactory, ExecutorServiceDelegate, SaneRejectedExecutionHandler, CarrierThreadFactory, VirtualizedExecutorService
- `classic::dispatch/affinity (4)`: RejectionHandler, RejectionHandlerFactory, QueueSelectorFactory, QueueSelector
- `classic::event (58)`: Register, Unregister, DeadLetterListener, EventBus, ActorEventBus, ActorClassifier, PredicateClassifier, LookupClassification, SubchannelClassification, ScanningClassification, ManagedActorClassification, EventStream, Register, UnregisterIfNoMoreSubscribedChannels, LoggerMessageQueueSemantics, LoggingBus, DummyClassForStringSources, LogSource, Logging, LogLevel, LoggerException, LogEventException, LogEvent, LogEventWithCause, Error, Error2, Error3, NoCause, Warning, Warning2, Warning3, Warning4, Info, Info2, Info3, Debug, Debug2, Debug3, LogEventWithMarker, InitializeLogger, LoggerInitialized, LoggerInitializationException, StdOutLogger, StandardOutLogger, DefaultLogger, LoggingAdapter, LoggingFilter, LoggingFilterWithMarker, LoggingFilterWithMarkerWrapper, DefaultLoggingFilter, DiagnosticLoggingAdapter, LogMarker, MarkerLoggingAdapter, DiagnosticMarkerBusLoggingAdapter, BusLogging, NoLogging, NoMarkerLogging, LoggingReceive
- `classic::io (127)`: BufferPool, Dns, Command, DnsExt, Settings, IpVersionSelector, IO, Extension, Inet, SocketOption, AbstractSocketOption, SocketOptionV2, AbstractSocketOptionV2, DatagramChannelCreator, SO, ReceiveBufferSize, ReuseAddress, SendBufferSize, TrafficClass, SoForwarders, SoJavaFactories, InetAddressDnsProvider, InetAddressDnsResolver, SelectionHandlerSettings, HasFailureMessage, WorkerForCommand, Retry, ChannelConnectable, ChannelAcceptable, ChannelReadable, ChannelWritable, SimpleDnsCache, SimpleDnsManager, Tcp, SO, KeepAlive, OOBInline, TcpNoDelay, Message, Command, Connect, Bind, Register, Unbind, CloseCommand, Close, ConfirmedClose, Abort, NoAck, WriteCommand, SimpleWriteCommand, Write, WritePath, CompoundWrite, ResumeWriting, SuspendReading, ResumeReading, ResumeAccepting, Event, Received, Connected, CommandFailed, WritingResumed, Bound, Unbound, ConnectionClosed, Closed, Aborted, ConfirmedClosed, PeerClosed, ErrorClosed, TcpExt, Settings, TcpSO, TcpMessage, PendingBufferWrite, PendingWriteFile, ReadResult, EndOfStream, AllRead, MoreDataWaiting, CloseInformation, ConnectionInfo, UpdatePendingWriteAndThen, WriteFileFailed, Unregistered, PendingWrite, EmptyPendingWrite, RegisterIncoming, FailedRegisterIncoming, Udp, Message, Command, NoAck, Send, Bind, Unbind, SimpleSender, SuspendReading, ResumeReading, Event, Received, CommandFailed, Bound, SimpleSenderReady, Unbound, SO, Broadcast, UdpExt, UdpMessage, UdpSO, UdpConnected, Message, Command, NoAck, Send, Connect, Disconnect, SuspendReading, ResumeReading, Event, Received, CommandFailed, Connected, Disconnected, UdpConnectedExt, UdpConnectedMessage
- `classic::io/dns (23)`: CachePolicy, Never, Forever, Ttl, DnsProtocol, RequestType, Ip, Srv, Resolve, Resolved, ResourceRecord, ARecord, AAAARecord, CNameRecord, SRVRecord, UnknownRecord, DnsSettings, Policy, ThreadLocalRandom, SecureRandom, EnhancedDoubleHashRandom, RecordClass, RecordType
- `classic::pattern (32)`: AskTimeoutException, AskSupport, ExplicitAskSupport, AskableActorRef, ExplicitlyAskableActorRef, AskableActorSelection, ExplicitlyAskableActorSelection, BackoffOpts, BackoffOnStopOptions, BackoffOnFailureOptions, BackoffSupervisor, GetCurrentChild, CurrentChild, Reset, GetRestartCount, RestartCount, CircuitBreaker, CircuitBreakerOpenException, CircuitBreakersRegistry, FutureTimeoutSupport, GracefulStopSupport, Patterns, PipeToSupport, PipeableFuture, PipeableCompletionStage, FutureRef, PromiseRef, RetrySupport, StatusReply, ErrorMessage, Success, Error
- `classic::routing (61)`: BalancingPool, BroadcastRoutingLogic, BroadcastPool, BroadcastGroup, ConsistentHash, ConsistentHashingRouter, ConsistentHashable, ConsistentHashableEnvelope, ConsistentHashMapper, ConsistentHashingRoutingLogic, ConsistentHashingPool, ConsistentHashingGroup, ListenerMessage, Listen, Deafen, WithListeners, Listeners, MurmurHash, OptimalSizeExploringResizer, DefaultOptimalSizeExploringResizer, RandomRoutingLogic, RandomPool, RandomGroup, Resizer, ResizerInitializationException, DefaultResizer, Resize, RoundRobinRoutingLogic, RoundRobinPool, RoundRobinGroup, RouterActorCreator, RoutingLogic, Routee, ActorRefRoutee, ActorSelectionRoutee, NoRoutee, SeveralRoutees, Router, Broadcast, RouterEnvelope, RouterConfig, GroupBase, Group, Pool, PoolBase, CustomRouterConfig, FromConfig, NoRouter, GetRoutees, Routees, AddRoutee, RemoveRoutee, AdjustPoolSize, ScatterGatherFirstCompletedRoutingLogic, ScatterGatherFirstCompletedPool, ScatterGatherFirstCompletedGroup, SmallestMailboxRoutingLogic, SmallestMailboxPool, TailChoppingRoutingLogic, TailChoppingPool, TailChoppingGroup
- `classic::serialization (21)`: AsyncSerializer, AsyncSerializerWithStringManifest, AsyncSerializerWithStringManifestCS, Serialization, Settings, Information, SerializationExtension, SerializationSetup, SerializerDetails, Serializer, Serializers, SerializerWithStringManifest, ByteBufferSerializer, BaseSerializer, JSerializer, NullSerializer, JavaSerializer, CurrentSystem, DisabledJavaSerializer, JavaSerializationException, ByteArraySerializer
