# actor モジュール ギャップ分析

更新日: 2026-03-25

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 約 65 |
| fraktor-rs 対応公開型数 | 約 52 |
| カバレッジ（主要公開型・主要 facade 単位） | 約 52/65 (80%) |
| ギャップ数 | 13（hard: 1, medium: 7, easy: 5） |

※ 今回の計数は「概念があるか」ではなく、`fraktor-rs` の**公開 API 面で Pekko の主要 facade / 主要公開型に直接対応しているか**を優先して保守的に数えている。  
※ `javadsl.*`、`private[pekko]`、JVM 固有の `Deploy` / `DynamicAccess` / classic-typed ブリッジ専用 API は除外した。  
※ `type-per-file` により fraktor-rs 側の公開型数は Pekko より細かく増えるため、型総数ではなく「主要公開型・主要 facade 単位」で比較している。  

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 24 | 18 | 75% |
| core / typed ラッパー | 31 | 24 | 77% |
| std / アダプタ | 10 | 10 | 100% |

## カテゴリ別ギャップ

### Typed actor コア ✅ 実装済み 24/31 (77%)

実装済みの中心:

- `Behavior`, `Behaviors`, `TypedActorContext`, `TypedActorRef`, `TypedActorSystem`
- `TimerScheduler`, `StashBuffer`, `ActorRefResolver`, `SpawnProtocol`
- `SupervisorStrategy`, `BackoffSupervisorStrategy`, `ExtensionSetup`
- `DispatcherSelector`, `MailboxSelector`, `TypedProps`

ギャップ:

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RecipientRef[T]` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorRef.scala:126` | 未対応 | core/typed | easy | `TypedActorRef<M>` は存在するが、送信専用の共通基底 trait がない |
| `ActorRef.path` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorRef.scala:63` | 未対応 | core/typed | medium | `TypedActorRef` は `pid()` のみ公開。パスは `ActorRefResolver` 経由の補助手段に留まる |
| `ActorRef.unsafeUpcast` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorRef.scala:55` | 未対応 | core/typed | easy | `map<N>()` はあるが、Pekko の variance API そのものではない |
| `ActorSystem.systemActorOf` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:176` | 部分実装 | core/typed | medium | untyped では [`ExtendedActorSystem::spawn_system_actor`](../../../modules/actor/src/core/system/extended_actor_system.rs) があるが typed facade に出ていない |
| `ActorSystem.deadLetters[U]` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:154` | 未対応 | core/typed + core/system | medium | `dead_letters()` は snapshot を返すのみで、生きた `ActorRef` を返さない |
| `ActorSystem.ignoreRef[U]` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:159` | 未対応 | core/typed + core/system | easy | 明示的な ignore ref がない |
| `ActorSystem.printTree` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:167` | 未対応 | core/system | medium | ツリー可視化 API が facade にない |
| `ActorSystem.address` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:199` | 部分実装 | core/system | medium | `canonical_authority()` / canonical path はあるが、typed system の単純 API としては未公開 |
| typed extensions facade (`registerExtension`, `hasExtension`, `extension`) | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Extensions.scala:152` | 部分実装 | core/typed + core/system | medium | untyped `ExtendedActorSystem` にはあるが、typed facade に直接出ていない |

### Typed DSL / Context / Lifecycle ✅ 実装済み 14/16 (88%)

実装済みの中心:

- `Behaviors.setup`, `with_stash`, `receive_message`, `receive_message_partial`, `receive_partial`
- `receive_signal`, `with_timers`, `intercept`, `transform_messages`, `monitor`
- `ActorContext.watch`, `watch_with`, `unwatch`, `child`, `children`
- `message_adapter`, `spawn_message_adapter`, `pipe_to_self`, `ask`, `ask_with_status`, `schedule_once`

ギャップ:

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorContext.setLoggerName(name/class)` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/ActorContext.scala:99` | 未対応 | std/typed | easy | std 側 `LogOptions` はあるが、context 上の logger 名変更 API はない |
| `spawnAnonymous` の dedicated API | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/ActorContext.scala:134` | 部分実装 | core/typed | easy | 無名 spawn は内部では可能だが、`spawn_child` に対する dedicated facade がない |

### Receptionist / Router / Topic / Delivery ✅ 実装済み 8/10 (80%)

実装済みの中心:

- `Receptionist`, `ServiceKey`, `Listing`
- `GroupRouterBuilder`, `PoolRouterBuilder`
- `Topic`
- `ProducerController`, `ConsumerController`, `WorkPullingProducerController`

ギャップ:

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Receptionist.Registered` / `Deregistered` ACK API | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/receptionist/Receptionist.scala:173` | 未対応 | core/typed | medium | `Register` / `Deregister` コマンド自体はあるが、ACK オブジェクトと reply-to 付き契約がない |
| `DurableProducerQueue` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/delivery/DurableProducerQueue.scala` | 未対応 | core/typed + persistence | hard | reliable delivery の永続化キューが欠落。`persistence` との統合が必要 |

### Classic runtime / dispatch / mailbox / scheduler ✅ 実装済み 8/10 (80%)

実装済みの中心:

- `Actor`, `ActorContext`, `ActorRef`, `ActorPath`, `Address`
- `ActorSystem`, `ExtendedActorSystem`
- `Dispatchers` registry, `DispatcherConfig`, `Mailbox`, `Mailboxes`
- bounded / priority / stable-priority / deque / control-aware mailbox 群
- `Scheduler`, `SchedulerHandle`, `PinnedDispatcher`

ギャップ:

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| typed facade としての `Dispatchers.lookup/defaultExecutionContext` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Dispatchers.scala:38` | 部分実装 | core/typed + core/dispatch | medium | registry と config はあるが、typed actor 利用者向けの lookup facade はない |
| `MailboxSelector.unbounded` の明示 API | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Props.scala:208` | 未対応 | core/typed | easy | `Default` はあるが、unbounded を明示する selector がない |

### Pattern / Serialization / Event / Extension ✅ 実装済み 9/11 (82%)

実装済みの中心:

- `CircuitBreaker`
- `StatusReply`
- serializer 群、`Serializer`, `SerializerWithStringManifest`, `AsyncSerializer`
- `SerializationRegistry`, `SerializationSetup`, `SerializationExtension`
- `EventStream`, `LogEvent`, `LoggerSubscriber`
- extension 基盤

ギャップ:

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| 汎用 `EventBus` / classification API 群 | `references/pekko/actor/src/main/scala/org/apache/pekko/event/EventBus.scala:33` | 未対応 | core/event | medium | fraktor-rs は `EventStream` と logging に寄っており、Pekko の分類型 event bus は未提供 |
| `LogOptions.withLogger` / `getLogger` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/LogOptions.scala:43` | 部分実装 | std/typed | easy | `with_logger_name` はあるが logger instance 注入はない |

## 主要な「別名で実装済み」

以下は名前は違うが、機能の中心契約は概ね満たしているためギャップには数えていない。

| Pekko API | fraktor-rs 対応 |
|-----------|-----------------|
| `Behaviors.receiveMessageWithSame` | `receive_message(... Ok(Behaviors::same()))` |
| `ActorContext.pipeToSelf` | `TypedActorContext::pipe_to_self` |
| `ActorContext.askWithStatus` | `TypedActorContext::ask_with_status` |
| `Routers.group/pool` | `GroupRouterBuilder` / `PoolRouterBuilder` |
| `StatusReply.ack()` | `StatusReply::ack()` |
| classic `Cancellable` | `SchedulerHandle` |
| `ActorRefResolver.toSerializationFormat/resolveActorRef` | `core/typed/actor_ref_resolver.rs` |

## スタブ / 未完成実装

`modules/actor/src` に対して `todo!()` / `unimplemented!()` を検索した範囲では、公開 API 直下のスタブ痕跡は見つからなかった。

## 内部モジュール構造ギャップ

今回は API ギャップが支配的なため省略。

省略理由:

- `medium` / `hard` の API ギャップが 8件残っている
- typed facade 不足と reliable delivery の永続化欠落が、内部構造差分より優先度が高い
- まず公開契約の欠落を埋めないと、構造比較の結論が実装優先度に直結しにくい

## 実装優先度の提案

### Phase 1: easy（公開 facade の穴埋め）

- `TypedActorRef.path`
- `TypedActorRef.unsafeUpcast` 相当
- `MailboxSelector.unbounded`
- `LogOptions.withLogger` / `getLogger`
- `spawnAnonymous` facade

### Phase 2: medium（typed facade の整備）

- `TypedActorSystem.systemActorOf`
- `TypedActorSystem.deadLetters` / `ignoreRef`
- `TypedActorSystem.printTree`
- `TypedActorSystem.address`
- typed extension facade
- `Receptionist.Registered` / `Deregistered` ACK
- typed `Dispatchers.lookup` facade
- 汎用 `EventBus` の扱いをどうするか判断

### Phase 3: hard（モジュール横断）

- `DurableProducerQueue`
  - `typed/delivery` と `persistence` の接続が必要
  - 再送、確認応答、クラッシュ復旧時の順序保証を設計する必要がある

### 対象外（n/a）

- `javadsl.*`, `AbstractBehavior`, `BehaviorBuilder`, `ReceiveBuilder`
- JVM 固有の `Deploy`, `DynamicAccess`, classic/typed bridge 専用 API

## まとめ

- `actor` モジュールは **typed behavior / typed context / router / topic / delivery / mailbox 群** までかなり広く揃っている
- 一方で **typed facade の細部** と **durable delivery** はまだ Pekko に追いついていない
- 今回の最大ギャップは `DurableProducerQueue` で、これは単純な型追加ではなく `persistence` 連携を伴う
- その手前の段階として、`TypedActorRef` / `TypedActorSystem` の facade を詰めると API カバレッジが大きく改善する
- API ギャップがまだ支配的なので、内部モジュール構造ギャップ分析は後続フェーズが妥当
