# actor モジュール ギャップ分析

更新日: 2026-06-11

## 比較スコープ定義

actor は現行の分割済みクレートを parity 単位にする。前回（2026-05-18）は粗い概念粒度（分母 114）で 100% と判定したが、今回は Pekko 側公開契約を細粒度（分母 272）で再集計した。分母の変更はスコープ変更ではなく粒度変更である。

| 層 | fraktor-rs 側 | Pekko 側 | 扱い |
|----|---------------|----------|------|
| kernel | `modules/actor-core-kernel/src/` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/`, `.../routing/`, `.../pattern/`, `.../event/`, `.../dispatch/`（Mailbox/Dispatcher 契約のみ） | classic / untyped actor runtime 契約 |
| typed | `modules/actor-core-typed/src/` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/`（scaladsl + top-level。javadsl 除外） | typed actor API と typed runtime 契約 |
| std adaptor | `modules/actor-adaptor-std/src/` | Pekko の dispatcher / scheduler / logging 実装のうち Rust std adapter として意味を持つ契約 | tokio / thread / clock / tracing 等の adapter 実装 |
| embassy adaptor | `modules/actor-adaptor-embassy/src/` | 対応なし（fraktor 独自の no_std 組込み層） | parity 分母に入れない。存在のみ記録 |

対象外（n/a）として分母から除外したもの:

| 対象外 | 理由 |
|--------|------|
| Java DSL / Java interop: `AbstractActor`, `UntypedAbstractActor`, `AbstractFSM`, `AbstractActorWithTimers`, `ReceiveBuilder`, `javadsl/*`, `japi/*`, `AbstractExtensionId`, `Patterns`（Java facade） | Java 継承 DSL / builder DSL。Rust では trait / builder / typed API に置き換える |
| Scala 構文糖: `Actor.Receive` / `emptyBehavior` / `ignoringBehavior`（PartialFunction 値）、`receivePartial` / `receiveMessagePartial` / `receiveMessageWithSame`、`ScalaActorSelection`、`AskableActorRef`（implicit ラッパー）、`pattern/extended`、`LogSource`（implicit 型クラス） | Rust API として同型にする必要がない。機能自体は別形で対応済み |
| JVM 固有: `DynamicAccess` / `ReflectiveDynamicAccess`、`ProviderSelection` / configurator 系（HOCON ロード）、`IndirectActorProducer`（リフレクション生成）、`DispatcherPrerequisites`、`JavaSerializer` / `DisabledJavaSerializer`、`NoSerializationVerificationNeeded`（serialize-messages 検証）、`JvmExitReason`、JFR | JVM 実装方式に依存する |
| `LightArrayRevolverScheduler`、`dungeon/*`、typed `internal/*`、`EventStreamUnsubscriber`、`Deployer` / `RepointableActorRef` | `private[pekko]` / `@InternalApi`。公開契約ではない（内部構造比較では参照する） |
| `PromiseRef` / `FutureRef` | JVM Promise イディオム。fraktor では ask 内部機構が同等責務を担う |
| deprecated: classic remoting、`BalancingDispatcher`（deprecated だが fraktor は実装済みのため対応済み扱い）、`ActorDSL`、`TypedActor` | deprecated / Pekko 参照ツリーに不在 |
| Pekko IO / TCP / UDP / DNS | transport / network adapter の別スコープ。`modules/actor-core-kernel/src/io.rs` は private な名前空間予約 placeholder のため分母に入れない |
| `*-tests`, `*-testkit`, `*-tck`, `src/test`, `multi-jvm` | testkit 調査は明示されていないため除外 |

raw declaration count は参考値であり、parity 分母には使わない。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 272 |
| fraktor-rs 固定スコープ対応概念 | 246 |
| 固定スコープ概念カバレッジ | 246/272 (90%) |
| raw Pekko type-like declarations | 1,085 参考値（actor src/main 全体: 807、actor-typed src/main: 278。javadsl/japi 除外、io / serialization / util 等の対象外パッケージ込み） |
| raw Pekko def declarations | 4,542 参考値（classic: 3,549、typed: 993） |
| raw Rust public type declarations | 624 参考値（kernel: 463, typed: 135, std: 20, embassy: 6。`*_test.rs` 除外） |
| raw Rust public fn declarations | 2,581 参考値（kernel: 1,923, typed: 608, std: 33, embassy: 17） |
| hard / medium / easy / trivial gap | 0 / 5 / 7 / 2 |
| `todo!()` / `unimplemented!()` / `panic!("not implemented")` | 0 件（kernel / typed / std / embassy すべて） |
| placeholder | 1 件（`actor-core-kernel/src/io.rs`。意図的な名前空間予約で parity 分母外） |

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| kernel | classic actor core, FSM, supervision, DeathWatch, dispatch/mailbox, routing, event, pattern, scheduler, serialization, extension, shutdown, setup | 主要契約は到達可能。FSM / pipe / BackoffSupervisor / Setup 類も実装済み | 残ギャップは EventBus 分類 trait 群と mailbox 設定契約に集中 |
| typed | typed ref/system/behavior/interceptor/context, signal, StashBuffer, router, receptionist, pubsub, delivery, ask/StatusReply, timers | `pipe_to_self` / `ctx.ask` / `with_mdc` / `monitor` / `log_messages` / `print_tree` / `ignore_ref` / `DeathPactError` まで確認。スタブ 0 | typed surface は実質 100%。ReceptionistSetup 相当の差し替え口のみ未対応 |
| std adaptor | executor, scheduler driver, clock, tracing logging, circuit breaker registry | Tokio/Threaded/Pinned/Affinity executor, Std/Tokio/Test tick driver, StdClock, TracingLoggerSubscriber, CircuitBreakersRegistry, StdBlocker | core/std 境界は妥当 |
| embassy adaptor | （Pekko 対応なし） | EmbassyExecutor(Driver/Factory), EmbassyTickDriver, embassy 用 clock/config。スタブ 0 | fraktor 独自層。parity 対象外だが健全 |

## カテゴリ別ギャップ

ギャップ（未対応・部分実装）のみテーブルに列挙する。実装済みはカテゴリの件数カウントに含めるが行には載せない。

### classic actor core　✅ 実装済み 30/34 (88%)

`Actor`, `ActorContext`, `Props`, `ActorSystem`, `ExtendedActorSystem`, stash（bounded/unbounded + `StashOverflowError`）, `Timers`（`ClassicTimerScheduler`）, `PoisonPill` / `Kill` / `Identify` / `ActorIdentity` / `ReceiveTimeout` / `NotInfluenceReceiveTimeout` / `Terminated` / `UnhandledMessage`, `Status`, FSM 本体（`Fsm<State, Data>` の `when` / `start_with` / `on_transition` / `on_termination` / 名前付きタイマー / `LoggingFsm`）は kernel に存在する。Pekko 側でも classic `Stash` は `StashOverflowException` を投げるのみで `StashOverflowStrategy` は persistence スコープのため、actor スコープのギャップではない。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `PossiblyHarmful` marker | `actor/Actor.scala` | 未対応 | core/kernel | trivial | remote untrusted mode で危険メッセージを遮断するためのマーカー。remote 連携時に必要 |
| `FSM.CurrentState` / `SubscribeTransitionCallBack` / `UnsubscribeTransitionCallBack` | `actor/FSM.scala` | 部分実装 | core/kernel | easy | `machine.rs:150` の `on_transition` はクロージャ観測のみ。外部アクターが遷移を購読するメッセージプロトコルがない（`FsmTransition` 型自体は存在） |

### supervision / lifecycle / DeathWatch　✅ 実装済み 10/13 (77%)

`SupervisorStrategy`（OneForOne / AllForOne / Directive）, `RestartLimit` / `RestartStatistics`, `BackoffSupervisor` + `BackoffOnFailureOptions` / `BackoffOnStopOptions`, DeathWatch（watch / unwatch / watch_with / `DeathWatchNotification`）, parent termination completion（`finish_terminate`）, `AddressTerminated` 統合, `DeadLetter`, `Dropped`（`DeadLetterReason` variant）は存在する。`AllDeadLetters` は「dead letter ストリームが reason 込みで全種を流す」別設計で対応済みとみなす。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `DeadLetterSuppression` marker + `SuppressedDeadLetter` 生成経路 | `actor/ActorRef.scala:564` | 部分実装 | core/kernel | easy | `dead_letter_reason.rs:21` に variant はあるが生成箇所が存在しない。ユーザーメッセージが抑制を宣言する契約も未配線 |
| `WrappedMessage` | `actor/ActorRef.scala` | 未対応 | core/kernel | trivial | dead letter / event stream 購読者がラップ済みメッセージを unwrap するための trait |

### dispatch / mailbox　✅ 実装済み 30/35 (86%)

`Executor` / `ExecutorFactory`, `Mailbox`, `MailboxType`, `MessageQueue`, `Envelope`, `MailboxPolicy`, `MailboxOverflowStrategy`, `Dispatchers` registry, bounded / unbounded / deque / priority / stable-priority / control-aware の queue family, `MessagePriorityGenerator`, `BalancingDispatcher` は kernel に存在する。`NonBlockingBoundedMailbox` は現行 bounded（drop 系戦略）が同等。control-aware bounded の overflow 形状差（MBX-L1: normal queue 優先 evict による control 保護）は同目的の別設計として対応済み扱い。詳細は `docs/gap-analysis/actor-mailbox-gap-analysis.md` を併読。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `pushTimeOut` 付き blocking bounded mailbox（`BoundedMailbox` / `BoundedPriorityMailbox` / `BoundedStablePriorityMailbox` / `BoundedDequeBasedMailbox` / `BoundedControlAwareMailbox`） | `dispatch/Mailbox.scala` | 部分実装 | core/kernel + std | medium | MBX-H1。fraktor の bounded は Drop 系のみで「満杯時に待つ」契約がない。async-first 方針で意図的に低優先度だが parity ギャップとしては残存 |
| `RequiresMessageQueue[T]` / `ProducesMessageQueue[T]` + queue semantics marker 群 | `dispatch/Mailbox.scala` | 部分実装 | core/kernel | medium | MBX-M1。`mailbox_requirement.rs` の capability 検証はあるが、actor 宣言ベースの queue type 解決モデルがない |
| `Mailboxes.lookupByQueueType` / 多段 mailbox selection precedence | `dispatch/Mailboxes.scala` | 部分実装 | core/kernel | medium | MBX-M2。deploy → dispatcher config → actor requirement → default の多段解決と `bounded-capacity:` 相当 helper がない。現状は `mailbox_id` か `MailboxConfig` の単純解決 |
| `BalancingDispatcherConfigurator` の mailbox 互換検証契約 | `dispatch/Dispatchers.scala` | 部分実装 | core/kernel | medium | MBX-M3。multiple-consumer 互換チェックが dispatcher 内部実装（`SharedMessageQueue`）に吸収され、mailbox type 契約として露出しない |

### classic routing　✅ 実装済み 36/36 (100%)

RoundRobin / Random / Broadcast / SmallestMailbox / ConsistentHashing の logic + pool, `Pool` / `Group` / `Routee` / `Router` / `RoutingLogic` / `RouterConfig` / `CustomRouterConfig` / `RemoteRouterConfig` / `RemoteRouterPool`, 管理メッセージ（`RouterCommand` / `RouterResponse` ≈ GetRoutees / AddRoutee / RemoveRoutee / AdjustPoolSize）, `Listeners` / `Listen` / `Deafen` / `WithListeners` を kernel に持つ。ScatterGatherFirstCompleted / TailChopping / BalancingPool / `Resizer` / `DefaultResizer` / `OptimalSizeExploringResizer` は **typed 層**（`actor-core-typed/src/dsl/routing/`）に同等セマンティクスで存在するため対応済みとする（Pekko は classic 側に置く。層配置差は内部構造の節を参照）。`FromConfig` / `NoRouter` は HOCON 駆動のため n/a。

### event / logging　✅ 実装済み 13/21 (62%)

`EventStream`, subscriber 群, `DeadLetter` 系, `UnhandledMessage`, `LoggingAdapter` / `DiagnosticActorLogging` / `BusLogging` / `LoggingReceive` / `LoggingFilter` / `DefaultLoggingFilter` / `NoLogging` / `LogLevel` / `ActorLogMarker`（marker 対応 LoggingAdapter）は存在する。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| 汎用 EventBus trait 族（`EventBus` / `ActorEventBus` / `ActorClassifier` / `PredicateClassifier` / `LookupClassification` / `SubchannelClassification` / `ScanningClassification` / `ManagedActorClassification`） | `event/EventBus.scala` | 未対応 | core/kernel | medium | 8 概念。fraktor の分類は `ClassifierKey` enum に固定されており、ユーザー定義のイベントバス / 分類戦略を構築する拡張契約がない。Pekko の `EventStream` 自体が `SubchannelClassification` 上に構築されている |

### pattern　✅ 実装済み 13/16 (81%)

classic `ask`（timeout 込み）, `pipe_to` / `pipe_to_self`, `retry`, `graceful_stop`（メッセージ指定版含む）, `CircuitBreaker` + std の `CircuitBreakersRegistry`, `BackoffSupervisor` / `BackoffOpts` 相当, typed `AskPattern` / `StatusReply` / `StatusReplyError` / `ask_with_status` は存在する。`ExplicitAskSupport` は typed の reply-to ファクトリ形 ask が同等。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FutureTimeoutSupport.after` | `pattern/FutureTimeoutSupport.scala:25` | 未対応 | core/kernel | easy | scheduler を使った遅延 future ヘルパー。timeout は `ask_with_timeout` 内部にのみ存在 |
| `AskableActorSelection` | `pattern/AskSupport.scala:431` | 未対応 | core/kernel | easy | ActorSelection に対する resolve + ask の合成。現状 ask は `ActorRef` のみ（`pattern/ask.rs:22`） |
| `CircuitBreaker` の `onOpen` / `onClose` / `onHalfOpen` リスナーと `withExponentialBackoff` / `withRandomFactor` | `pattern/CircuitBreaker.scala:133` | 部分実装 | core/kernel | easy | 状態遷移（Closed/Open/HalfOpen）は追跡するが、遷移リスナー登録と reset timeout の指数バックオフ / ジッタがない（固定 timeout） |

### scheduling / timers　✅ 実装済み 6/6 (100%)

kernel `Scheduler`（timer wheel ベース）, `Cancellable`（registry 込み）, `ClassicTimerScheduler`（single / fixed-delay / fixed-rate / `is_timer_active` / `cancel` / `cancel_all`）, receive timeout, typed `Scheduler`, typed `TimerScheduler`（同 6 操作）すべて存在。

### ref / resolution　✅ 実装済み 13/13 (100%)

`ActorRef`, `ActorPath`（Root / Child / parser / formatter）, `Address`, `ActorSelection` + resolver, typed `ActorRef` / `RecipientRef`, `ActorRefResolver` / `ActorRefResolverSetup` すべて存在。`ClassicActorSystemProvider` / `ClassicActorContextProvider` は `from_untyped` / `into_untyped` ブリッジの別設計で対応済み。

### extension　✅ 実装済み 6/6 (100%)

kernel `Extension` / `ExtensionId` / `ExtensionInstaller` / `ExtensionInstallers`, typed `ExtensionSetup`, typed system の extension アクセスすべて存在。

### coordinated shutdown　✅ 実装済み 7/9 (78%)

`CoordinatedShutdown`（8 フェーズ定数 + `CoordinatedShutdownPhase`）, `CoordinatedShutdownReason`（Custom 含む）, installer / id / error, `run` / `shutdown_reason` は存在する。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `addCancellableTask` / `addActorTerminationTask` | `actor/CoordinatedShutdown.scala` | 部分実装 | core/kernel | easy | `coordinated_shutdown.rs:230` は `add_task` のみ。登録解除可能タスクと actor 停止待ちタスクの変種がない |

### setup　✅ 実装済み 4/5 (80%)

`Setup` 基底相当, `ActorSystemSetup`, `BootstrapSetup`, `ActorRefResolverSetup` は存在する。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ReceptionistSetup` | `actor-typed/.../receptionist/Receptionist.scala` | 未対応 | core/typed | easy | typed system は `install_system_receptionist`（`system.rs:181`）でローカル receptionist を固定インストールする。clustered receptionist 等への差し替え契約がない。cluster 側の clustered receptionist 実装自体は cluster スコープ |

### typed core surface　✅ 実装済み 57/57 (100%)

`Behavior` / `BehaviorDirective`, `ExtensibleBehavior`, `BehaviorInterceptor`（around_receive / around_signal / around_start）+ signal interceptor, `AbstractBehavior`, Behaviors ファクトリ全種（setup / receive / receive_message / receive_signal / same / stopped / empty / ignore / unhandled / with_timers / with_stash / supervise / monitor / log_messages / intercept / transform_messages / with_mdc 系 3 種）, `LogOptions`, signal 群（PreRestart / PostStop / Terminated / ChildFailed / MessageAdaptionFailure / `DeathPactError`）, typed supervision（resume / restart / stop / restart_with_backoff, `RestartSupervisorStrategy`, `BackoffSupervisorStrategy`）, typed `ActorContext`（self / system / spawn / spawn_anonymous / stop / watch / unwatch / children / `pipe_to_self` / `ask` / `ask_with_status` / message_adapter / schedule_once / set_receive_timeout / stash / delegate / forward）, `StashBuffer`（全 API）, `TypedActorSystem`（`ignore_ref` 相当 / `print_tree`（`system.rs:374`）/ systemActorOf 相当 / dead letters / event stream / receptionist アクセス）, `TypedActorSystemConfig` / `TypedActorSystemLog`, `Dispatchers` / `DispatcherSelector` / `MailboxSelector` / `Props` / `ActorTags`, `SpawnProtocol`, typed routing（Routers / PoolRouter / GroupRouter + scatter_gather / tail_chopping / balancing / Resizer 系）すべて存在。classic↔typed adapter package は「typed が kernel 上に直接構築され `from_untyped` / `into_untyped` で双方向変換できる」別設計で対応済み。

### receptionist　✅ 実装済み 6/6 (100%)

`ServiceKey`, `Receptionist` extension, `Register` / `Deregister` / `Subscribe` / `Find`, `Registered` / `Deregistered` / `Listing`（`services_were_added_or_removed` 差分フラグ込み）すべて存在。差し替え口（ReceptionistSetup）は setup カテゴリに計上。

### typed eventstream　✅ 実装済み 5/5 (100%)

`EventStreamCommand`（Publish / Subscribe / Unsubscribe）存在。

### typed pubsub　✅ 実装済み 6/6 (100%)

`Topic` / `TopicCommand`（Publish / Subscribe / Unsubscribe / GetTopicStats）/ `TopicStats` 存在。参照 Pekko ツリーの typed pubsub は `Topic.scala` のみで、PubSub registry は分母に含めない。

### typed delivery　✅ 実装済み 4/4 (100%)

`ProducerController`, `ConsumerController`, `WorkPullingProducerController`, `DurableProducerQueue`（State / MessageSent / confirmation 系込み）存在。

### std adaptor　✅ 実装済み 6/6 (100%)

`TokioExecutor` / `TokioTaskExecutor`, `ThreadedExecutor`, `PinnedExecutor`, `AffinityExecutor`（各 Factory 込み。ThreadedExecutor のみ Factory 型なしで直接構築）, `StdTickDriver` / `TokioTickDriver` / `TestTickDriver`, `StdClock` + monotonic mailbox clock, `TracingLoggerSubscriber`, `DeadLetterLogSubscriber`, `CircuitBreakersRegistry`, `StdBlocker`, `PanicInvokeGuard`。スタブ 0。

## 内部モジュール構造ギャップ

API カバレッジ 90%・hard/medium 未実装 5 件以下のため、内部モジュール構造ギャップ分析を実施した。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 問題の種類 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|-----------|----------------|--------|--------|------|
| ActorCell の dungeon facet 未分離 | `actor/dungeon/`（Dispatch / FaultHandling / DeathWatch / Children / ReceiveTimeout を独立 trait に分離、`ActorCell.scala` は 733 行のオーケストレータ） | `actor/actor_cell.rs` 1,809 行に 5 責務 + stash + timers + pipe が混在（`pub(crate)` メソッド 38 個）。テストも 2,389 行のモノリス | 未分離 | 同一型の `impl` ブロックをファイル分割し、fault_handling / death_watch / children / dispatch / receive_timeout の facet 単位へ再編 | medium | high | Rust でも同一クレート内なら impl 分割で実現可能。変更頻度が高い箇所 |
| SystemState のゴッドオブジェクト化 | `ActorSystem` は `Dispatchers` / `Mailboxes` / `EventStream` / guardian 等の独立サブシステムへ委譲 | `system/state/system_state.rs` 1,147 行 + `system_state_shared.rs` 1,094 行に dispatcher registry / cell table / guardian / serialization / remote hook / scheduler が同居 | 未分離 | 関心ごとのサブ struct（registry 単位）へ分割し、SystemState は束ね役に縮小 | medium | medium | ActorCell と同型の問題。dispatcher 追加が serialization と同一ファイル変更になる |
| `SystemMessage` の層配置 | `dispatch/sysmsg/SystemMessage.scala`（dispatch 層の所有） | `actor/messaging/system_message.rs`（actor ドメイン配下）。dispatch/mailbox が actor 側へ依存して取得 | 誤配置 | `dispatch/` 配下へ責務移動し、依存方向を dispatch → actor から actor → dispatch へ正す | trivial | low | 概念上の層違反。机上では移動のみだが re-export 調整が必要 |
| typed 層が薄いラッパーを超えている | typed `internal/adapter/*` は classic への委譲に徹し、receptionist は `LocalReceptionist`（behavior 実装）と extension API を分離 | `system.rs` 610 行に `IgnoreRefSender` / `EventStreamRefSender` 等の実装が同居。`receptionist/extension.rs` 534 行に extension API と behavior 実装が同居。`delivery/internal/work_pulling_producer_controller.rs` 1,385 行 | 責務混在 | facade と behavior 実装をファイル分離（extension API / behavior 実装 / 内部 sender）。delivery の flow-control プリミティブの kernel 降格を検討 | medium | medium | ReceptionistSetup（API ギャップ）導入時に分離すると一石二鳥 |
| ReceiveTimeout 責務の分散 | `dungeon/ReceiveTimeout.scala` 84 行に完結 | `receive_timeout_state.rs` / `receive_timeout_state_shared.rs` / `actor_cell.rs` 内 7 箇所 / `actor_context.rs` に分散 | 責務分散 | ActorCell facet 分割と同時に receive_timeout facet へ集約 | easy | low | ActorCell 分割の一部として実施可能 |
| kernel レベルの interceptor 拡張点不足 | typed `InterceptorImpl` + dungeon trait による facet 差し替え | `MessageInvokerMiddleware` と `InvokeGuard` のみ。テレメトリ等の横断関心を cell 改変なしに注入する点が狭い | 拡張点不足 | `MessageInvokerMiddleware` の適用範囲を文書化し、必要になった時点で cell-level hook を追加 | easy | low | typed `BehaviorInterceptor` は存在するため実害は限定的 |
| kernel public surface の広さ | public API と `private[pekko]` を明確に分離 | `actor.rs` が `ActorCell` / `ActorShared` / `ChildRef` 等の低レベル型を public re-export | 過剰公開 | 外部契約に必要な型と `pub(crate)` へ落とせる型を棚卸し | medium | low | 前回からの繰越。pre-release なので破壊的整理は可能 |

構造ギャップにしないもの: classic routing の高位ルーター（ScatterGather / TailChopping / BalancingPool / Resizer）が typed 層にのみある点は、fraktor が typed を主 API とする方針下で責務境界が明確なため許容する。embassy adaptor の機能面の薄さ（event / pattern 不在）は no_std 制約による意図的な絞り込みであり構造ギャップではない。

## 実装優先度

ここで出す優先度は「Pekko parity ギャップをどの順で埋めるか」であり、YAGNI は適用しない。各項目はカテゴリ別ギャップからの再掲である。

### Phase 1: trivial / easy

- `FSM.CurrentState` / `SubscribeTransitionCallBack` / `UnsubscribeTransitionCallBack` 遷移購読プロトコル（core/kernel）
- `DeadLetterSuppression` marker + `SuppressedDeadLetter` 生成経路の配線（core/kernel）
- `CircuitBreaker` 状態遷移リスナー + `withExponentialBackoff` / `withRandomFactor`（core/kernel）
- `CoordinatedShutdown` の `addCancellableTask` / `addActorTerminationTask` 相当（core/kernel）
- `ReceptionistSetup` 相当の receptionist 差し替え契約（core/typed）
- `FutureTimeoutSupport.after` 相当の遅延 future ヘルパー（core/kernel）
- `AskableActorSelection` 相当の selection ask（core/kernel）
- `PossiblyHarmful` marker（core/kernel）
- `WrappedMessage`（core/kernel）

### Phase 2: medium

- 汎用 EventBus trait 族（Lookup / Subchannel / Scanning / ManagedActor classification）（core/kernel）
- `RequiresMessageQueue[T]` / `ProducesMessageQueue[T]` 相当の queue type 宣言と解決（core/kernel）
- `lookupByQueueType` / 多段 mailbox selection precedence（core/kernel）
- `BalancingDispatcher` の mailbox 互換契約の外部化（core/kernel）
- `pushTimeOut` 付き blocking bounded mailbox 契約（core/kernel + std。async-first 方針との調停が必要）

### Phase 3: hard

該当なし。

### 対象外（n/a）

- Java DSL / `javadsl/*` / `japi/*` / `AbstractActor` 系 / `Patterns`
- Scala 構文糖（PartialFunction 系 receive 変種、implicit ラッパー、`LogSource`）
- JVM reflection / classloader / HOCON loading / configurator 系 / serialize-messages 検証
- Java serialization / JFR / `JvmExitReason`
- `PromiseRef` / `FutureRef`、deprecated（classic remoting / `ActorDSL` / `TypedActor`）
- Pekko IO / TCP / UDP / DNS（transport の別スコープ）
- testkit / TCK / tests

## まとめ

actor モジュールの固定スコープ概念カバレッジは 246/272 (90%) である。前回（2026-05-18）の 114/114 (100%) は粗い概念粒度での判定であり、細粒度で再集計した結果、未実装・部分実装 26 概念（テーブル行 14 件）が残る。スタブ（`todo!` 等）は 4 クレートすべてで 0 件であり、存在する API の実装品質は高い。

低コストで parity を前進できるのは Phase 1 の 9 件（FSM 遷移購読、dead letter 抑制配線、CircuitBreaker リスナー、CoordinatedShutdown タスク変種、ReceptionistSetup、`after`、selection ask、マーカー trait 2 種）。主要ギャップは Phase 2 の 5 件で、汎用 EventBus 分類 trait 族と mailbox 設定契約（requirement 解決 / selection precedence / blocking bounded / balancing 互換）に集中している。hard 級ギャップは存在しない。

API ギャップが 1 桁 medium まで縮んだ現在、次のボトルネックは公開 API ではなく内部構造にある。特に `actor_cell.rs`（1,809 行）の dungeon facet 分離と `system_state.rs`（1,147 + 1,094 行）の分割が、今後の変更速度と保守性を左右する。typed 層の facade / behavior 実装分離は ReceptionistSetup 導入と同時に行うのが効率的である。
