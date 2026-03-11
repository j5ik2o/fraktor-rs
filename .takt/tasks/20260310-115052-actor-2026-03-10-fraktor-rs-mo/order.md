# actor モジュール ギャップ分析

> 分析日: 2026-03-10
> 対象:
> - fraktor-rs: `modules/actor/src/`
> - Pekko (classic): `references/pekko/actor/src/main/scala/org/apache/pekko/actor/`
> - Pekko (typed): `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/`

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（classic + typed、JVM固有を除く） | 約 83 |
| fraktor-rs 公開型数 | 約 89 |
| カバレッジ（概念単位） | 約 65% |
| ギャップ数（主要） | 22 |

> **注記**:
> - Pekko は classic（非型付き）と typed の二系統を持つ。fraktor-rs は両方に対応しているが、classic 側のカバレッジが低い。
> - JVM 固有の型（`DynamicAccess`、`ReflectiveDynamicAccess`、`IndirectActorProducer`、`AbstractActor` 等）は `n/a` として除外している。
> - fraktor-rs は 1機能を複数の小型公開型へ分割する設計のため公開型数は Pekko より多くなる。同名型の一致率だけでは過小評価になる。

---

## カテゴリ別ギャップ

### 1. 基本型・アクター定義

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Actor` (trait) | `Actor.scala:482` | `core::actor::actor_lifecycle::Actor` | - | 実装済み |
| `ActorContext` | `ActorCell.scala:57` | `core::actor::actor_context::ActorContext` | - | 実装済み |
| `ActorRef` | `ActorRef.scala:116` | `core::actor::actor_ref::ActorRef` | - | 実装済み |
| `Props` | `Props.scala:124` | `core::props::Props` | - | 実装済み |
| `PoisonPill` / `Kill` | `Actor.scala:52,67` | `SystemMessage` 経由で実装済み | - | 実装済み（`ActorRef::poison_pill`/`kill`） |
| `Identify` + `ActorIdentity` | `Actor.scala:81,91` | 未対応 | easy | アクター発見プロトコル。classic API で汎用的に使われる |
| `ReceiveTimeout` (classic) | `Actor.scala:154` | 部分対応 | medium | typed の `set_receive_timeout` は実装済み。classic 側のみ未対応 |
| `Status.Failure` / `Status.Success` (classic) | `Actor.scala:313-326` | typed 側は `StatusReply` で対応 | easy | classic 側の `Status` enum は未対応 |
| `UnhandledMessage` | `Actor.scala:298` | `core::typed::UnhandledMessageEvent` | - | typed 側で実装済み |

---

### 2. アクターシステム・プロバイダー

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ActorSystem` | `ActorSystem.scala:527` | `core::system::ActorSystem` | - | 実装済み |
| `ExtendedActorSystem` | `ActorSystem.scala:732` | `core::system::ExtendedActorSystem` | - | 実装済み |
| `ActorRefFactory` | `ActorRefProvider.scala:189` | `core::system::provider::ActorRefProvider` | - | 別名で実装済み |
| `CoordinatedShutdown` | `CoordinatedShutdown.scala:41` | 未対応 | hard | フェーズ制御付きシャットダウン。依存モジュールとの連携が必要 |

---

### 3. アクターパス・アドレス

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ActorPath` (sealed) | `ActorPath.scala:171` | `core::actor::actor_path::ActorPath` | - | 実装済み |
| `RootActorPath` | `ActorPath.scala:278` | `GuardianKind` として内部実装 | - | 実装済み |
| `Address` | `Address.scala` | `core::system::remote::RemotingConfig` | medium | 独立した `Address` 型が未実装。リモーティング連携に必要 |
| `ActorSelection` | `ActorSelection.scala:39` | `core::actor::actor_selection::ActorSelectionResolver` | medium | パス文字列による参照解決のみ。`ActorSelection` のメッセージ送信が未対応 |

---

### 4. 監視戦略（Supervision）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `SupervisorStrategy` (classic) | `FaultHandling.scala:320` | `core::supervision::SupervisorStrategy` | - | 基本型は実装済み（設計が異なる） |
| `OneForOneStrategy` | `FaultHandling.scala:579` | 未対応 | medium | 一子専用の監視戦略。現在はデフォルト戦略のみ |
| `AllForOneStrategy` | `FaultHandling.scala:465` | 未対応 | hard | 全子に影響する監視戦略 |
| `BackoffSupervisorStrategy` | `FaultHandling.scala` | `core::supervision::BackoffSupervisorStrategy` | - | 実装済み |
| `SupervisorStrategy` (typed) | `SupervisorStrategy.scala:237` | `core::supervision::SupervisorStrategy` | - | 実装済み |
| `RestartSupervisorStrategy` | `SupervisorStrategy.scala:251` | 部分実装 | easy | 基本は実装済み。`withCriticalLogLevel` は n/a の可能性 |

---

### 5. スケジューラー・キャンセラブル

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Scheduler` | `Scheduler.scala:59` | `core::scheduler::Scheduler` | - | 実装済み |
| `Cancellable` | `Scheduler.scala:456` | `core::scheduler::CancellableEntry` | - | 実装済み（別名） |
| `TaskRunOnClose` | `Scheduler.scala:498` | `core::scheduler::task_run::TaskRunOnClose` | - | 実装済み |
| `LightArrayRevolverScheduler` | `LightArrayRevolverScheduler.scala:51` | 未対応 | n/a | 内部実装。fraktor では別の tick 機構で代替 |

---

### 6. メールボックス・ディスパッチャー

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Mailbox` + `MailboxType` | `Mailboxes.scala` | `core::dispatch::mailbox::Mailbox` + `MailboxType` | - | 実装済み |
| `Dispatcher` + `Dispatchers` | `Dispatcher.scala` | `core::dispatch::dispatcher::Dispatchers` | - | 実装済み |
| `BoundedMailbox` / `UnboundedMailbox` | `Mailboxes.scala` | `core::dispatch::mailbox::BoundedMailboxType` 等 | - | 実装済み |
| `PinnedDispatcher` | `PinnedDispatcher.scala` | 未対応 | medium | スレッド固定ディスパッチャー。`TokioExecutor`/`ThreadedExecutor` で代替可 |
| `BalancingDispatcher` | `BalancingDispatcher.scala` | 未対応 | hard | ワークスティーリング型。Rust の async ランタイムで代替可能 |

---

### 7. イベントストリーム・デッドレター

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `EventStream` | `ActorSystem.scala` | `core::event::stream::EventStream` | - | 実装済み |
| `DeadLetter` | `ActorRef.scala:564` | `core::dead_letter::DeadLetter` | - | 実装済み |
| `AllDeadLetters` | `ActorRef.scala:551` | `core::dead_letter::DeadLetterReason` | - | 概念は対応 |
| `SuppressedDeadLetter` / `Dropped` | `ActorRef.scala:582,594` | 未対応 | trivial | `DeadLetterReason` enum に追加するだけ |

---

### 8. 拡張（Extension）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Extension` | `Extension.scala:74` | `core::extension::Extension` | - | 実装済み |
| `ExtensionId` | `Extension.scala:81` | `core::extension::ExtensionId` | - | 実装済み |
| `ExtensionIdProvider` | `Extension.scala:137` | `core::extension::ExtensionInstaller` | - | 概念は別名で実装済み |

---

### 9. FSM・Stash・Timers (classic)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `FSM` (classic) | `FSM.scala:430` | 未対応 | hard | classic FSM は実装なし。typed の `FsmBuilder` のみ |
| `LoggingFSM` | `FSM.scala:937` | 未対応 | hard | FSM の拡張 |
| `FsmBuilder` (typed) | fraktor 独自 | `core::typed::FsmBuilder` | - | fraktor 独自の typed FSM 実装 |
| `Timers` (classic) | `Timers.scala:31` | 未対応 | medium | typed の `TimerScheduler` は実装済み。classic 側は未対応 |
| `Stash` (classic) | `Stash.scala:71` | 未対応 | medium | classic 側未対応。typed の `StashBuffer<M>` は実装済み |
| `UnboundedStash` | `Stash.scala:76` | 未対応 | easy | Stash が実装されれば trivial |
| `StashBuffer` (typed) | `StashBuffer.scala` | `core::typed::StashBuffer` | - | 実装済み |

---

### 10. パターン API

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ask(actorRef, timeout)` | `Patterns.scala:78` | `ActorRef::ask` | - | 実装済み（`ask_with_timeout` あり） |
| `ask(actorSelection, timeout)` | `Patterns.scala:237` | 未対応 | medium | `ActorSelection` 実体不足に依存 |
| `pipeTo` / `pipeToSelection` | `PipeToSupport.scala:31` | `pipe_to_self` | medium | 他 actor への pipe は未対応 |
| `gracefulStop` | `GracefulStopSupport.scala:59` | 未対応 | medium | `terminate` はあるが対象 actor 単位 graceful stop がない |
| `BackoffSupervisor` / `BackoffOpts` | `BackoffSupervisor.scala:22` | `BackoffSupervisorStrategy` | - | 実装済み（オプション DSL 互換は未提供） |
| `RetrySupport` | `RetrySupport.scala:30` | 未対応 | easy | 補助ユーティリティとして切り出し可能 |
| `CircuitBreaker` / `CircuitBreakersRegistry` | `CircuitBreaker.scala:133` | 未対応 | hard | actor 境界を超える責務増が大きい |

---

### 11. Typed API（actor-typed）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Behavior[T]` | `Behavior.scala` | `core::typed::Behavior<M>` | - | 実装済み |
| `ActorRef[T]` (typed) | `ActorRef.scala` | `core::typed::actor::TypedActorRef<M>` | - | 実装済み |
| `ActorSystem[T]` (typed) | `ActorSystem.scala` | `core::typed::TypedActorSystem<M>` | - | 実装済み |
| `ActorContext[T]` (scaladsl) | `scaladsl/ActorContext.scala` | `core::typed::actor::TypedActorContext<M>` | - | 実装済み |
| `Behaviors` (全ファクトリー) | `scaladsl/Behaviors.scala` | `core::typed::Behaviors` | - | ほぼ実装済み（下記を除く） |
| `Behaviors.receive(ctx, msg)` | `Behaviors.scala:115` | 未対応 | easy | `receive_message` はあるが `receive(ctx, msg)` 形式がない |
| `Behaviors.logMessages` | `Behaviors.scala:215` | 未対応 | trivial | `monitor` と `intercept` で代替可能 |
| `Behaviors.withMdc` | `Behaviors.scala:285` | 未対応 | n/a | JVM の MDC ログ機能。Rust では `tracing` で代替 |
| `ExtensibleBehavior` | `Behavior.scala:106` | 未対応 | medium | カスタム Behavior 基底クラス |
| `BehaviorInterceptor` (クラス) | `BehaviorInterceptor.scala:44` | `intercept` 関数のみ | medium | クラスとして intercept を表現する型が未実装 |
| `SpawnProtocol` | `SpawnProtocol.scala:36` | 未対応 | easy | アクター経由でのスポーンプロトコル |
| `ActorRefResolver` (typed) | `ActorRefResolver.scala` | 未対応 | medium | typed ActorRef のシリアライズ・デシリアライズ用 |
| Signal 型（PreRestart/PostStop/Terminated/ChildFailed/MessageAdaptionFailure） | `MessageAndSignals.scala` | `core::typed::BehaviorSignal` | - | 実装済み（全シグナル対応） |
| `SupervisorStrategy` (typed) + `RestartSupervisorStrategy` + `BackoffSupervisorStrategy` | `SupervisorStrategy.scala` | `core::supervision::SupervisorStrategy` + `BackoffSupervisorStrategy` | - | 実装済み |
| `Receptionist` + `ServiceKey` + `Listing` | `receptionist/Receptionist.scala` | `core::typed::Receptionist` + `ServiceKey` + `Listing` | - | 実装済み |
| `Routers` (typed) | `actor-typed` | `core::typed::Routers` | - | 実装済み |
| `TimerScheduler` (typed) | `Scheduler.scala` | `core::typed::TimerScheduler` | - | 実装済み |
| `StatusReply` | `actor-typed` | `core::typed::StatusReply` | - | 実装済み |
| `StashBuffer` (typed) | `StashBuffer.scala` | `core::typed::StashBuffer` | - | 実装済み |

---

### 12. Pub/Sub・Delivery（actor-typed）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Topic` (pubsub) | `pubsub/Topic.scala` | 未対応 | medium | typed pub/sub パターン。`EventStream` で代替可能だが型付きでない |
| `ProducerController` | `delivery/ProducerController.scala` | 未対応 | hard | フロー制御付きメッセージ配信。persistence モジュールと連携が必要 |
| `ConsumerController` | `delivery/ConsumerController.scala` | 未対応 | hard | フロー制御付きメッセージ受信 |
| `WorkPullingProducerController` | `delivery/WorkPullingProducerController.scala` | 未対応 | hard | ワークプーリングパターン |
| `DurableProducerQueue` | `delivery/DurableProducerQueue.scala` | 未対応 | hard | 永続化付きプロデューサーキュー（persistence 依存） |

---

## 実装優先度の提案

- untyped(fraktor-rsではclassicとは言わない。カーネル部分。ユーザ部分は型付きでラップする方式なっている)からtypedに依存しないこと
- カーネル部分から実装するのが得策
- untypedでしか使わない機能やモジュールは優先度を落としてよい

### Phase 1: trivial（既存組み合わせで即実装可能）

- `Behaviors.logMessages` — `monitor` + `intercept` のラッパー
- `SuppressedDeadLetter` / `Dropped` — `DeadLetterReason` enum への追加

### Phase 2: easy（単純な新規実装）

- `Identify` / `ActorIdentity` — classic 発見プロトコル。シンプルなメッセージ型
- `Status.Failure` / `Status.Success` (classic) — typed の `StatusReply` と並行して追加
- `ReceiveTimeout` (classic) — typed 側の実装を参照
- `Behaviors.receive(ctx, msg)` — `receive_message` にコンテキスト引数を追加
- `SpawnProtocol` — typed アクターのスポーンプロトコル
- `UnboundedStash` (classic) — Stash が実装されれば trivial
- `RetrySupport` — 補助ユーティリティとして追加
- `RestartSupervisorStrategy` 追加設定 — `withStopChildren`/`withStashCapacity` 等

### Phase 3: medium（中程度の実装工数）

- `OneForOneStrategy` (classic) — classic 監視の基本戦略
- `Stash` (classic) — typed `StashBuffer` の classic 版
- `Timers` (classic) — typed `TimerScheduler` の classic 版
- `ActorSelection` 送信機能 — 現在はリゾルバーのみ
- `Address` 型 — リモーティング連携に必要
- `BehaviorInterceptor` クラス — 現在は `intercept` 関数のみ
- `ExtensibleBehavior` — カスタム Behavior 基底
- `PinnedDispatcher` — スレッド固定ディスパッチャー
- `Topic` (pub/sub) — typed pub/sub
- `ActorRefResolver` (typed) — シリアライズ連携
- `gracefulStop` — 対象 actor 単位の graceful stop
- `pipeTo` (他 actor へ) — `pipe_to_self` の拡張

### Phase 4: hard（アーキテクチャ変更を伴う）

- `AllForOneStrategy` (classic) — 全子停止の監視戦略
- `CoordinatedShutdown` — フェーズ制御シャットダウン
- `FSM` (classic) — classic FSM トレイト
- `BalancingDispatcher` — ワークスティーリング型ディスパッチャー
- `CircuitBreaker` + registry
- `ProducerController` / `ConsumerController` / `WorkPullingProducerController` — フロー制御配信
- `DurableProducerQueue` — 永続化付きキュー（persistence 依存）

### 対象外（n/a）

- `AbstractActor` / `UntypedAbstractActor` — Java クライアント向け。Rust 不要
- `DynamicAccess` / `ReflectiveDynamicAccess` — JVM リフレクション固有
- `IndirectActorProducer` — JVM クラスローダー固有
- `Behaviors.withMdc` — JVM MDC ログ固有。`tracing` で代替
- `LogOptions` — JVM ロギング固有
- `LightArrayRevolverScheduler` — Pekko 内部実装。fraktor では tick 機構で代替
- `Deploy` / `Scope` / `LocalScope` — リモートデプロイ設定（remote モジュールで扱う）
- `LoggingFSM` — FSM が実装されたときに検討
- Java DSL 固有サーフェス（`AbstractActor` 系 Java API 互換の完全再現）

---

## 根拠（主要参照）

- Pekko classic:
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Actor.scala`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorSelection.scala:39`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Stash.scala:71`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FSM.scala:430`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FaultHandling.scala:320`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/CoordinatedShutdown.scala:41`

- Pekko typed:
  - `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Behaviors.scala`
  - `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/ActorContext.scala`
  - `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/SupervisorStrategy.scala`
  - `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala`
  - `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/BehaviorInterceptor.scala`
  - `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/SpawnProtocol.scala`
  - `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/delivery/`
  - `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/pubsub/Topic.scala`

- fraktor-rs:
  - `modules/actor/src/core/actor/actor_ref/base.rs`
  - `modules/actor/src/core/actor/actor_selection/resolver.rs`
  - `modules/actor/src/core/typed/behaviors.rs`
  - `modules/actor/src/core/typed/actor/actor_context.rs`
  - `modules/actor/src/core/typed/behavior_signal.rs`
  - `modules/actor/src/core/supervision/`
  - `modules/actor/src/core/typed/stash_buffer.rs`
  - `modules/actor/src/core/typed/fsm_builder.rs`
