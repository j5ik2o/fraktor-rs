# actor モジュール ギャップ分析

## 前提と集計範囲

- 比較対象:
  - fraktor-rs: `modules/actor/src/core`, `modules/actor/src/std`
  - Pekko: `references/pekko/actor/src/main/scala/org/apache/pekko/actor`, `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed`
- 集計対象は parity に直接関係する公開型・主要 DSL に限定する。
- `io`、`japi`、`util`、`testkit`、Java DSL 専用 API は今回の集計対象から除外した。
- 型数はオーバーロードを 1 件に集約した概数。fraktor-rs 側は「対応する公開型があるか」を、ギャップ表は「契約まで満たしているか」を見ている。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 約 42 |
| fraktor-rs 公開型数 | 約 32（core: 29, std: 3） |
| カバレッジ（型単位） | 約 32/42 (76%) |
| ギャップ数 | 18（core: 15, std: 3） |

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 15 | 14 | 93% |
| core / typed ラッパー | 20 | 13 | 65% |
| std / アダプタ | 7 | 5 | 71% |

## カテゴリ別ギャップ

### classic / untyped コア　✅ 実装済み 9/12 (75%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorContext.setReceiveTimeout` / `cancelReceiveTimeout` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorCell.scala:L103` | 未対応 | core/kernel | medium | `modules/actor/src/core/kernel/actor/actor_context.rs` は `spawn/watch/stop` を持つが classic 文脈の receive-timeout API はない |
| `Cancellable` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/Scheduler.scala:L456` | 部分実装 | core/kernel | easy | `modules/actor/src/core/kernel/actor/scheduler/handle.rs:9` の `SchedulerHandle` と `.../cancellable/cancellable_entry.rs:77` の `is_cancelled` で近いが、Pekko の `Cancellable` 契約をそのまま公開していない |
| `CoordinatedShutdown` の extension-style entrypoint | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/CoordinatedShutdown.scala:L190`, `:L697` | 部分実装 | std | easy | `modules/actor/src/std/system/coordinated_shutdown.rs:96-234` は本体を持つが、`apply/get` で引ける extension 入口がない |

### typed 基本型 / DSL　✅ 実装済み 9/16 (56%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorRef.narrow` / `unsafeUpcast` / `path` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorRef.scala:L47`, `:L55`, `:L63` | 部分実装 | core/typed | easy | `modules/actor/src/core/typed/actor_ref.rs:31-128` は `map` と `into_untyped` を持つ。`path` は untyped 側 `modules/actor/src/core/kernel/actor/actor_ref/base.rs:74` にあるが typed surface に露出していない |
| `Behaviors.stopped(postStop)` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Behaviors.scala:L91` | 未対応 | core/typed | easy | `modules/actor/src/core/typed/dsl/behaviors.rs:106` は引数なし `stopped()` のみ |
| `Behaviors.receiveMessageWithSame` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Behaviors.scala:L154` | 未対応 | core/typed | trivial | `receive_message` はあるが「戻り値なしで same を返す」薄い sugar がない |
| `AbstractBehavior` / `Receive` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/AbstractBehavior.scala:L46`, `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Behaviors.scala:L330` | 未対応 | core/typed | medium | fraktor-rs は関数ベース DSL に寄っており、継承ベース / builder ベース DSL がない |
| `Signal.PostStop` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/MessageAndSignals.scala:L59` | 部分実装 | core/typed | medium | `modules/actor/src/core/typed/behavior_signal.rs:10-28` は `Started`/`Stopped`/`PreRestart` を持つが `PostStop` 名義の parity はない |
| `ActorContext.spawnAnonymous` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/ActorContext.scala:L134` | 部分実装 | core/typed | medium | `modules/actor/src/core/typed/actor/actor_context.rs:102-114` は `spawn_child` / `spawn_child_watched` のみ。匿名 spawn は `SpawnProtocol` と unnamed props 経由で代替している |
| `ActorSystem.dispatchers` / `logConfiguration` / `Extensions` facade | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:L62`, `:L106`, `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/Extensions.scala:L160`, `:L166` | 部分実装 | core/typed | medium | `modules/actor/src/core/typed/system.rs:115-185` は `scheduler/event_stream/terminate` まで。拡張 API は `modules/actor/src/core/kernel/system/extended_actor_system.rs:76-93` 側にあるが typed surface に出ていない |

### typed discovery / pubsub / eventstream　✅ 実装済み 5/8 (63%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Receptionist.Registered` / `Deregistered` ack | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/receptionist/Receptionist.scala:L162`, `:L246` | 未対応 | core/typed | medium | `modules/actor/src/core/typed/receptionist.rs:148-188` は `register/deregister/subscribe/find` の command factory までで ACK 型がない |
| `Listing.isForKey` / `serviceInstance` / `serviceInstances` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/receptionist/Receptionist.scala:L175`, `:L188`, `:L378` | 部分実装 | core/typed | easy | `modules/actor/src/core/typed/receptionist/listing.rs:20-56` は `service_id/type_id/typed_refs` を返すが Pekko 互換の key-oriented accessor 群がない |
| `EventStream.Subscribe` / `Unsubscribe` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/eventstream/EventStream.scala:L52`, `:L68` | 部分実装 | core/typed | easy | `modules/actor/src/core/typed/eventstream/event_stream_command.rs:6` は `Publish` のみ。購読は `TypedActorSystem::subscribe_event_stream` で別 API に分かれている |

### typed routing / delivery　✅ 実装済み 4/9 (44%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `PoolRouter.withRouteeProps` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Routers.scala:L184` | 未対応 | core/typed | medium | `modules/actor/src/core/typed/dsl/routing/pool_router_builder.rs:55-113` は pool size / routing strategy / resizer までで routee props 差し替えがない |
| `GroupRouter.withRandomRouting(preferLocalRoutees)` / `withRoundRobinRouting(preferLocalRoutees)` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Routers.scala:L72`, `:L92` | 未対応 | core/typed | n/a | cluster locality 前提の API。現状 `modules/actor/src/core/typed/dsl/routing/group_router_builder.rs:47-61` はローカル receptionist 前提で、cluster parity は `modules/cluster` 連携が必要 |
| `ProducerController.Settings` と durable queue 系 builder | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/delivery/ProducerController.scala:L195-L234` | 部分実装 | core/typed | hard | `modules/actor/src/core/typed/delivery/producer_controller_settings.rs:10-17` は private な空設定。`modules/actor/src/core/typed/delivery/producer_controller.rs:119` も `behavior(producer_id)` のみ |
| `ConsumerController.Settings.withResendInterval*` | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/delivery/ConsumerController.scala:L249-L267` | 部分実装 | core/typed | medium | `modules/actor/src/core/typed/delivery/consumer_controller_settings.rs:19-47` は `flow_control_window` と `only_flow_control` だけ |
| `WorkPullingProducerController.Settings` の公開 tuning surface | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/delivery/WorkPullingProducerController.scala` | 部分実装 | core/typed | medium | `modules/actor/src/core/typed/delivery/work_pulling_producer_controller_settings.rs:12-28` は private かつ `buffer_size` の getter のみ。外部から設定を組み立てられない |

### std / runtime アダプタ　✅ 実装済み 5/7 (71%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Behaviors.withMdc(staticMdc, dynamicMdc)` 複合オーバーロード | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Behaviors.scala:L321` | 部分実装 | std | easy | `modules/actor/src/std/typed/behaviors.rs:222` の `with_mdc` と `:249` の `with_static_mdc` に分かれている |
| `CoordinatedShutdown` の system installer から見える一貫した入口 | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/CoordinatedShutdown.scala:L190`, `:L697` | 部分実装 | std | easy | `modules/actor/src/std/system/coordinated_shutdown_installer.rs:14` と `.../coordinated_shutdown.rs:96-234` が分離しており、Pekko ほど単純な `system -> coordinated shutdown` 導線になっていない |

## 内部モジュール構造ギャップ

今回は API ギャップが支配的なため省略する。  
型単位では coverage が見えているが、`typed` 表層の DSL・ACK・settings 契約がまだ不足しており、公開契約 parity の穴埋めが先行課題。

## 実装優先度

### Phase 1

- `Behaviors.receiveMessageWithSame` を追加する（core/typed）
- `Behaviors.stopped(postStop)` を追加する（core/typed）
- `TypedActorRef` に `path` を追加し、`narrow` / `unsafe_upcast` 相当の薄い API を揃える（core/typed）
- `Listing.is_for_key` / `service_instances` 相当を追加する（core/typed）
- `EventStreamCommand::Subscribe` / `Unsubscribe` を追加する（core/typed）
- `Cancellable` 互換の薄い公開契約を `SchedulerHandle` 上に揃える（core/kernel）
- `Behaviors.with_mdc(static + dynamic)` 複合オーバーロードを追加する（std）
- `CoordinatedShutdown` の extension-style entrypoint を揃える（std）

### Phase 2

- classic `ActorContext` に receive-timeout API を追加する（core/kernel）
- `AbstractBehavior` / `Receive` 相当の typed DSL を追加する（core/typed）
- `ActorContext.spawn_anonymous` を追加する（core/typed）
- `ActorSystem.dispatchers` / `log_configuration` / `extension` facade を typed surface に追加する（core/typed）
- `Receptionist.Registered` / `Deregistered` ACK 型を追加する（core/typed）
- `PoolRouter.withRouteeProps` を追加する（core/typed）
- `ConsumerController.Settings` の resend interval 系 builder を公開する（core/typed）
- `WorkPullingProducerController.Settings` の公開 builder を追加する（core/typed）

### Phase 3

- `ProducerController.Settings` の durable queue 系 builder と `behavior(..., settings)` を実装する（core/typed）

### 対象外（n/a）

- `GroupRouter` の `preferLocalRoutees` 系 API（core/typed）
  - cluster locality 前提であり、`modules/actor` 単体では完結しない

## まとめ

- 全体として、classic/untyped の基盤型と typed の主経路は揃っているが、Pekko parity の観点では `typed` 表層の DSL 完成度がまだ低い。
- 低コストで前進できるのは `receiveMessageWithSame`、`stopped(postStop)`、`EventStream.Subscribe/Unsubscribe`、`Listing` の key-oriented accessor、`Cancellable` 契約の薄い整備。
- 主要ギャップは `ProducerController.Settings` の durable queue 系、typed `AbstractBehavior/Receive`、typed `ActorSystem` の extension/dispatchers facade。
- 型の対応自体は進んでいるが、現時点では API ギャップが支配的であり、次のボトルネックは内部構造ではなく公開契約 parity にある。
