# actor モジュール ギャップ分析

## マッピング前提

このレポートでは比較軸を次の 3 つに固定する。

| Pekko 側 | fraktor-rs 側 | 備考 |
|----------|---------------|------|
| untyped (`pekko-actor`) | `modules/actor/src/core/kernel/` | classic / untyped の API family を `core/kernel` へ対応付ける |
| typed (`pekko-actor-typed`) | `modules/actor/src/core/typed/` | typed public surface を `core/typed` へ対応付ける |
| host/std runtime helper | `modules/actor/src/std/` | `CoordinatedShutdown` や IO family のような std 依存 API をここへ置く |

重要:

- 以前の版は classic 側を `ActorPath` / `Address` / `Cancellable` に縮退しており、`core/kernel` の残件数を把握するには不十分だった。
- 今回は **Pekko untyped -> core/kernel** を独立して数え、`core/kernel` の API 残件絶対数を先に出す。
- そのうえで、API family 数に現れない **構造差 / semantic 差** を別枠で記録する。

## core/kernel 絶対数サマリー

| 指標 | 値 |
|------|-----|
| Pekko untyped family 数（core/kernel 対応範囲） | 12 |
| core/kernel 実装済み family 数 | 12 |
| core/kernel API 未充足の絶対数 | 0 |
| core/kernel 構造 / semantic 差 | 5 |
| untyped 由来だが std 側で扱う family 数 | 2 |

解釈:

- **API family の絶対数としては `core/kernel` の残件は 0**。
- ただし、Pekko untyped と比較したときの **責務配置・設定モデル・semantic 差** は 5 件残っている。
- また、Pekko classic のうち `IO family` と `CoordinatedShutdown` advanced helpers は `core/kernel` ではなく `std` 側の課題として扱う。

## core/kernel family カバレッジ

### core/kernel ✅ 実装済み 12/12 (100%)

| family | Pekko参照 | fraktor対応 | 状態 | 備考 |
|--------|-----------|-------------|------|------|
| `ActorSystem` / `ActorRefFactory` surface | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorSystem.scala:527` | `modules/actor/src/core/kernel/system/base.rs:61` | 実装済み | actor selection, event stream, scheduler, actor 解決まで公開面がある |
| `ActorRefProvider` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorRefProvider.scala:40` | `modules/actor/src/core/kernel/actor/actor_ref_provider/base.rs:27` | 実装済み | local provider と installer 含む |
| `ActorSelection` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorSelection.scala:39` | `modules/actor/src/core/kernel/actor/actor_selection/selection.rs:24` | 実装済み | resolver 含む |
| `FSM` / `AbstractFSM` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FSM.scala:430` | `modules/actor/src/core/kernel/actor/fsm/machine.rs:33`, `modules/actor/src/core/kernel/actor/fsm/abstract_fsm.rs:6` | 実装済み | family として存在 |
| `Deploy` / `Deployer` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Deployer.scala:225` | `modules/actor/src/core/kernel/actor/deploy/descriptor.rs:8`, `modules/actor/src/core/kernel/actor/deploy/deployer.rs:10` | 実装済み | deploy 記述子と deployer を持つ |
| `Dispatchers` / `Dispatcher` runtime | `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Dispatchers.scala:114`, `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Dispatcher.scala:41` | `modules/actor/src/core/kernel/dispatch/dispatcher/dispatchers.rs:16`, `modules/actor/src/core/kernel/dispatch/dispatcher/dispatcher_shared.rs:28` | 実装済み | dispatcher family として存在 |
| `EventStream` | `references/pekko/actor/src/main/scala/org/apache/pekko/event/EventStream.scala:34` | `modules/actor/src/core/kernel/event/stream/base.rs:19` | 実装済み | shared handle と subscription を持つ |
| `Serialization` / `SerializationSetup` | `references/pekko/actor/src/main/scala/org/apache/pekko/serialization/Serialization.scala:147`, `references/pekko/actor/src/main/scala/org/apache/pekko/serialization/SerializationSetup.scala:46` | `modules/actor/src/core/kernel/serialization/extension.rs:48`, `modules/actor/src/core/kernel/serialization/serialization_setup.rs:17`, `modules/actor/src/core/kernel/serialization/serialization_registry/registry.rs:17` | 実装済み | registry / extension / setup を持つ |
| `Router` / `RoutingLogic` family | `references/pekko/actor/src/main/scala/org/apache/pekko/routing/RouterConfig.scala:52` | `modules/actor/src/core/kernel/routing/router.rs:22`, `modules/actor/src/core/kernel/routing/routing_logic.rs:14` | 実装済み | router 本体と routing logic family がある |
| `ActorPath` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorPath.scala` | `modules/actor/src/core/kernel/actor/actor_path/base.rs:13` | 実装済み | parser / formatter / child path を持つ |
| `Address` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Address.scala:38` | `modules/actor/src/core/kernel/actor/address.rs:17` | 実装済み | protocol / system / host / port を持つ |
| `Scheduler` / `Cancellable` bridge | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Scheduler.scala:456` | `modules/actor/src/core/kernel/actor/scheduler.rs:43`, `modules/actor/src/core/kernel/actor/scheduler/handle.rs:15` | 実装済み | `Cancellable` alias を追加済み |

## core/kernel 実装差異

以下は **API family の未充足数には含めない**。  
理由は、公開 family 自体は存在するが、Pekko と責務分割や semantic が一致していないため。

| 差異 | Pekko側の根拠 | fraktor-rs側の現状 | 種類 | 緊急度 | 備考 |
|------|---------------|--------------------|------|--------|------|
| `EventStream` の分類モデル差 | `references/pekko/actor/src/main/scala/org/apache/pekko/event/EventStream.scala:34` | `modules/actor/src/core/kernel/event/stream/base.rs:19` | semantic差 | medium | Pekko は `LoggingBus + SubchannelClassification` 前提、fraktor は明示購読レジストリ中心 |
| `Dispatchers` 設定モデル差 | `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Dispatchers.scala:114`, `references/pekko/actor/src/main/scala/org/apache/pekko/dispatch/Dispatcher.scala:41` | `modules/actor/src/core/kernel/dispatch/dispatcher/dispatchers.rs:16`, `modules/actor/src/core/kernel/dispatch/dispatcher/dispatcher_shared.rs:28` | semantic差 | medium | Pekko の configurator / config-driven executor ecosystem までは持たない |
| `Serialization` bootstrap 差 | `references/pekko/actor/src/main/scala/org/apache/pekko/serialization/Serialization.scala:147`, `references/pekko/actor/src/main/scala/org/apache/pekko/serialization/SerializationSetup.scala:46` | `modules/actor/src/core/kernel/serialization/extension.rs:48`, `modules/actor/src/core/kernel/serialization/builder.rs:20` | semantic差 | medium | Pekko の `DynamicAccess` / classloader / serializer 自動生成モデルとは一致しない |
| `Deployer` と router config の結合差 | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Deployer.scala:225` | `modules/actor/src/core/kernel/actor/deploy/deployer.rs:10`, `modules/actor/src/core/kernel/routing/` | 構造差 | medium | Pekko は deploy config から router config を組み立てるが、fraktor は責務が分離されている |
| remote 責務が actor core に同居 | Pekko remote は `pekko-remote` 側に分離 | `modules/actor/src/core/kernel/system/remote/` | 構造差 | high | `RemotingConfig` や `RemoteAuthorityRegistry` が `core/kernel` にある |

## std 側へ残る untyped runtime gap

これは Pekko classic 由来だが、`core/kernel` の残件数には入れない。

### std / untyped runtime helper ⏳ 0/2 実装済み

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Tcp` / `Udp` / `Dns` / `IO` family | Pekko actor module の public IO family 一式 | 未対応 | std | hard | `modules/actor/src/std/io/` は存在するが未公開で、公開 API parity には未到達 |
| `CoordinatedShutdown` advanced task helpers | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/CoordinatedShutdown.scala:564`, `:648`, `:815`, `:897` | 部分実装 | std | medium | `add_task` / `run` はあるが `addCancellableTask` / `addActorTerminationTask` 相当がない |

## typed 側の残件サマリー

core/kernel の絶対数は把握できたので、typed 側は残件だけを維持する。

| 指標 | 値 |
|------|-----|
| Pekko typed family 数（主集計） | 46 |
| core/typed 実装済み family 数 | 30 |
| core/typed API 未充足の絶対数 | 16 |

### typed 基盤 DSL / 型　✅ 実装済み 15/22 (68%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ExtensibleBehavior[T]` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Behavior.scala:106` | 未対応 | core/typed | hard | `AbstractBehavior` はあるが、`receive/receiveSignal` を持つ独立拡張点がない。 |
| `ActorRef.!` / `RecipientRef.!` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorRef.scala:77`, `:143` | 未対応 | core/typed | trivial | `tell` はある (`modules/actor/src/core/typed/actor_ref.rs:61`) が演算子エイリアスがない。 |
| `BehaviorInterceptor.isSame` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/BehaviorInterceptor.scala:92` | 未対応 | core/typed | trivial | fraktor の `BehaviorInterceptor` は `around_start/around_receive/around_signal` のみ (`modules/actor/src/core/typed/behavior_interceptor.rs:21`)。 |
| `Props.withMailboxFromConfig` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Props.scala:90` | 別経路のみ | core/typed | trivial | `MailboxSelector::from_config` (`modules/actor/src/core/typed/mailbox_selector.rs:40`) と `TypedProps::with_mailbox_selector` (`modules/actor/src/core/typed/props.rs:111`) はあるが、shorthand がない。 |
| `SupervisorStrategy.resume/restart/stop` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/SupervisorStrategy.scala:35`, `:44`, `:50` | 部分実装 | core/typed | medium | kernel の `SupervisorStrategy` はある (`modules/actor/src/core/kernel/actor/supervision/base.rs:23`) が、typed ルートの定数ファクトリがない。 |
| `RestartSupervisorStrategy` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/SupervisorStrategy.scala:266` | 部分実装 | core/typed | medium | `with_stop_children` / `with_stash_capacity` は kernel 側にある (`modules/actor/src/core/kernel/actor/supervision/base.rs:186`, `:193`) が、typed 専用型と `withLimit` がない。 |
| `BackoffSupervisorStrategy` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/SupervisorStrategy.scala:320` | 部分実装 | core/typed | medium | `BackoffSupervisorStrategy` 自体はある (`modules/actor/src/core/kernel/actor/supervision/backoff_supervisor_strategy.rs:18`) が、typed façade と Pekko と同じ builder surface ではない。 |

### typed ライフサイクル / Signals　✅ 実装済み 3/7 (43%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Signal` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala:33` | 未対応 | core/typed | medium | fraktor は `BehaviorSignal` enum で一括表現しており、marker trait がない (`modules/actor/src/core/typed/message_and_signals/signal.rs:10`)。 |
| `PreRestart` / `PostStop` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala:42`, `:52` | 部分実装 | core/typed | medium | enum variant としてはあるが、個別公開型ではない (`modules/actor/src/core/typed/message_and_signals/signal.rs:13`, `:21`)。 |
| `Terminated` / `ChildFailed` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala:81`, `:104` | 部分実装 | core/typed | medium | `Pid` ベースの enum variant で吸収しており、Pekko の dedicated wrapper 型ではない (`modules/actor/src/core/typed/message_and_signals/signal.rs:15`, `:19`)。 |
| `MessageAdaptionFailure` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala:125` | 部分実装 | core/typed | easy | variant はあるが、独立公開型ではない (`modules/actor/src/core/typed/message_and_signals/signal.rs:17`)。 |

### typed Receptionist / EventStream　✅ 実装済み 10/12 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Receptionist` extension façade (`ref`, `createExtension`, `get`) | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/receptionist/Receptionist.scala:33`, `:107`, `:108` | 部分実装 | core/typed | easy | fraktor は plain actor と `TypedActorSystem::receptionist_ref/receptionist` で提供 (`modules/actor/src/core/typed/receptionist.rs:42`, `modules/actor/src/core/typed/system.rs:252`)。ExtensionId としては公開していない。 |
| `ServiceKey.Listing` / `ServiceKey.Registered` extractor | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/receptionist/Receptionist.scala:81`, `:90` | 未対応 | core/typed | easy | `Listing` (`modules/actor/src/core/typed/receptionist/listing.rs:14`) と `Registered` (`modules/actor/src/core/typed/receptionist/registered.rs:16`) はあるが、`ServiceKey` に紐づく extractor helper がない。 |

### typed Routing / Delivery　✅ 実装済み 7/9 (78%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GroupRouter[T]` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Routers.scala:58` | 未対応 | core/typed | medium | `GroupRouterBuilder` はある (`modules/actor/src/core/typed/dsl/routing/group_router_builder.rs:29`) が、Pekko のような公開 Behavior 型はない。 |
| `PoolRouter[T]` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Routers.scala:131` | 未対応 | core/typed | medium | `PoolRouterBuilder` はある (`modules/actor/src/core/typed/dsl/routing/pool_router_builder.rs:26`) が、公開 Behavior 型がない。 |

## 実装優先度

### core/kernel

- API 残件 absolute count は 0
- 次の優先課題は API 追加ではなく、`EventStream` / `Dispatchers` / `Serialization` / `Deployer` / remote 境界の構造整理

### std

- `IO family`
- `CoordinatedShutdown` advanced helpers

### core/typed

- `ExtensibleBehavior`
- typed `SupervisorStrategy` façade
- dedicated signal types
- `GroupRouter` / `PoolRouter` public Behavior 型

## まとめ

- **Pekko untyped -> core/kernel の API family 絶対数は `12/12` で、残件 absolute count は `0`**。
- ただし `core/kernel` の実装差異は消えておらず、`構造 / semantic 差` として 5 件ある。
- `IO family` と `CoordinatedShutdown` advanced helpers は `core/kernel` の残件ではなく、`std` 側の untyped runtime gap。
- actor モジュール全体の次のボトルネックは `core/kernel` の API 欠損ではなく、`core/typed` の公開契約不足と `core/kernel` の責務配置差である。
