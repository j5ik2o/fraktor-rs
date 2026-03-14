# actor モジュール ギャップ分析

## 前提

- 比較対象:
  - fraktor-rs 側: `modules/actor/src`
  - Pekko 側: `references/pekko/actor/src`
- ただし、Pekko の `actor` モジュールは `org.apache.pekko.actor` だけでなく `routing` / `event` / `serialization` / `io` まで含み、fraktor-rs の `modules/actor` より守備範囲が広い。
- そのため、**生の公開型総数**は参考値に留め、**actor ドメインで直接比較可能な代表 public surface** を中心にギャップを整理する。
- `typed` については、Pekko では `actor-typed` が別モジュールであり、この比較では **対象外** とする。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（actor+routing の生 count） | 258 |
| fraktor-rs 公開型数（modules/actor の生 count） | 385 |
| カバレッジ（代表 actor surface） | 約 31/46 (67%) |
| ギャップ数 | 15 |

生 count では fraktor-rs 側が多いが、これは `core/std` 分離、typed API 同居、設定型・補助型の細分化による。
実質的な比較では、**基本 actor runtime はかなり揃っている一方、classic Pekko 特有の ActorSelection / deployment / CoordinatedShutdown / classic router 設定群が不足**している。

## カテゴリ別ギャップ

### Actor Core ✅ 実装済み 5/9 (56%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `PoisonPill` | `actor/Actor.scala:52` | 別名で実装済み | easy | fraktor は [`SystemMessage::PoisonPill`](../../modules/actor/src/core/messaging/system_message.rs) ベースで実装。公開 API 名は一致しない |
| `Kill` | `actor/Actor.scala:67` | 別名で実装済み | easy | fraktor は [`SystemMessage::Kill`](../../modules/actor/src/core/messaging/system_message.rs) |
| `Identify` / `ActorIdentity` | `actor/Actor.scala:81`, `actor/Actor.scala:91` | 実装済み | trivial | [`messaging::Identify`](../../modules/actor/src/core/messaging.rs), [`ActorIdentity`](../../modules/actor/src/core/messaging/actor_identity.rs) |
| `ReceiveTimeout` / `setReceiveTimeout` | `actor/Actor.scala:154`, `actor/ActorCell.scala:103` | 部分実装 | medium | typed 側の receive-timeout はあるが、classic untyped で Pekko 互換の公開 API にはなっていない |
| `become` / `unbecome` | `actor/Actor.scala`, `AbstractActor.scala` | 未対応 | hard | fraktor untyped actor は behavior stack を公開していない |

### ActorRef / Path / Selection ✅ 実装済み 5/8 (63%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ActorSelection` | `actor/ActorSelection.scala:35` | 部分実装 | medium | fraktor は [`ActorSelectionResolver`](../../modules/actor/src/core/actor/actor_selection/resolver.rs) まで。Pekko の selection handle API は未提供 |
| `ActorRef.forward` | `actor/ActorRef.scala:154` | 未対応 | easy | `tell` はあるが classic `forward` 相当の公開メソッドは見当たらない |
| `ActorRef.noSender` | `actor/ActorRef.scala:35` | 未対応 | trivial | sender 省略は可能だが、同名の sentinel API はない |

### ActorSystem / Bootstrap / Extension ✅ 実装済み 6/9 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `CoordinatedShutdown` | `actor/CoordinatedShutdown.scala:41`, `actor/ActorSystem.scala:663` | 未対応 | hard | terminate はあるが phase 付き coordinated shutdown はない |
| `ActorSystemSetup` / `BootstrapSetup` | `actor/ActorSystem.scala:41`, `actor/setup/ActorSystemSetup.scala:64` | 未対応 | medium | fraktor は [`ActorSystemConfig`](../../modules/actor/src/std/system/actor_system_config.rs) ベースで、setup 合成 DSL はない |
| `DynamicAccess` / `ReflectiveDynamicAccess` | `actor/DynamicAccess.scala`, `actor/ReflectiveDynamicAccess.scala` | 未対応 | n/a | JVM reflection 前提で、Rust では直接移植の価値が薄い |

### Props / Mailbox / Dispatcher ✅ 実装済み 5/9 (56%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Props.withDeploy` | `actor/Props.scala:204` | 未対応 | medium | fraktor Props は mailbox / dispatcher / name 中心で deploy オブジェクトを持たない |
| `Props.withRouter` | `actor/Props.scala:199` | 未対応 | medium | router は builder 側で構成し、Props に埋め込まない |
| `Props.withActorTags` | `actor/Props.scala:210` 付近 | 未対応 | easy | metadata tag API はない |
| mailbox id / dispatcher id からの classic 配置 | `actor/Props.scala:142-170` | 別名で実装済み | trivial | [`with_mailbox_id`](../../modules/actor/src/core/props/base.rs), [`with_dispatcher_id`](../../modules/actor/src/core/props/base.rs) |

### Supervision / Fault Handling ✅ 実装済み 4/7 (57%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `AllForOneStrategy` | `actor/FaultHandling.scala` | 部分実装 | medium | fraktor は strategy kind を持つが、classic API 名と builder surface はまだ薄い |
| `Escalate` を含む classic directive DSL | `actor/FaultHandling.scala` | 部分実装 | medium | core には outcome があるが、Pekko 互換の classic surface は限定的 |
| `preRestart` / `postRestart` classic hooks の完全互換 | `actor/Actor.scala` | 部分実装 | medium | fraktor は hook 群を持つが、classic lifecycle 契約は Pekko と完全一致ではない |

### Scheduler / Timers / Stash ✅ 実装済み 4/7 (57%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `scheduleAtFixedRate` | `actor/Scheduler.scala:188` 付近 | 未対応 | medium | fraktor は `schedule_once` / `schedule_with_fixed_delay` が中心 |
| classic `Timers` mixin | `actor/Timers.scala` | 未対応 | medium | typed / scheduler command で代替しているが classic mixin surface はない |
| `UnboundedStash` / `UnrestrictedStash` | `actor/Stash.scala:71-78` | 未対応 | medium | fraktor は typed `StashBuffer` が中心で classic trait ベースではない |

### Routing ✅ 実装済み 2/6 (33%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RoundRobinPool` / `RoundRobinGroup` classic config objects | `routing/RoundRobin.scala:83`, `routing/RoundRobin.scala:148` | 部分実装 | medium | fraktor は typed builder (`PoolRouterBuilder` / `GroupRouterBuilder`) で提供 |
| `BroadcastPool` / `BroadcastGroup` | `routing/Broadcast.scala:73`, `routing/Broadcast.scala:137` | 部分実装 | medium | fraktor は pool 側 broadcast predicate はあるが classic config object はない |
| `ConsistentHashingPool` / `ConsistentHashingGroup` | `routing/ConsistentHashing.scala:311`, `routing/ConsistentHashing.scala:385` | 部分実装 | medium | fraktor は rendezvous hash ベースの typed builder のみ |
| `Resizer` | `routing/Resizer.scala:40` | 未対応 | hard | 動的 routee resize は未実装 |

### Event / Logging ✅ 実装済み 3/5 (60%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `LoggingAdapter` / `DiagnosticLoggingAdapter` | `event/Logging.scala:1203`, `event/Logging.scala:1635` | 部分実装 | medium | fraktor は event stream と subscriber はあるが、Pekko の adapter 階層までは未対応 |
| `DeadLetterListener` | `event/DeadLetterListener.scala:33` | 未対応 | easy | dead letter store はあるが listener actor の classic surface はない |

### 対象外（n/a）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `IO/Tcp/Udp/Dns` 系 | `io/Tcp.scala`, `io/UdpConnected.scala`, `io/Dns.scala` | 対象外 | n/a | fraktor-rs の actor モジュールだけではなく remote/network サブシステムの領域 |
| Java/Scala utility API | `japi/JavaAPI.scala`, `util/ByteString.scala` | 対象外 | n/a | JVM/Scala 標準 API への適応層 |
| `serialization` 詳細 API | `serialization/Serialization.scala`, `Serializer.scala` | 部分比較のみ | n/a | Rust の serde/bincode ベース設計とは責務境界が異なる |
| `actor-typed` 相当 | `references/pekko/actor-typed/**` | 別比較対象 | n/a | 今回の対象は `references/pekko/actor` のみ |

## 実装優先度の提案

### Phase 1: trivial

- `ActorRef.forward` 相当の追加
- `ActorRef.noSender` 相当の明示 API 追加
- `DeadLetterListener` 相当の公開 listener surface 追加

### Phase 2: easy

- classic `ReceiveTimeout` の公開 API 整備
- `Props.withActorTags` 相当の軽量 metadata
- classic router surface へ typed builder の薄い adapter を追加

### Phase 3: medium

- `ActorSelection` を resolver だけでなく handle API まで引き上げる
- `Props.withDeploy` / `withRouter` の責務を Rust 流に再設計して導入する
- `Broadcast*` / `RoundRobin*` / `ConsistentHashing*` の classic surface を整理する
- `UnboundedStash` / `UnrestrictedStash` に相当する classic stash 契約を追加する

### Phase 4: hard

- `CoordinatedShutdown` の phase model
- dynamic `Resizer`
- `become` / `unbecome` を含む classic behavior stack

### 対象外（n/a）

- `io` / `serialization` / `japi` の JVM 依存 API
- `actor-typed` モジュールに属する typed surface

## まとめ

- 全体として、**fraktor-rs の actor モジュールは「基本 actor runtime」「mailbox/dispatcher 設定」「event stream」「scheduler 基盤」まではかなり揃っている**。
- すぐ価値を出せる不足は、`ActorRef.forward`、classic `ReceiveTimeout`、薄い classic router surface で、いずれも既存基盤の組み合わせで寄せやすい。
- 実用上の大きなギャップは、`ActorSelection` の公開 handle、`CoordinatedShutdown`、`Resizer`、classic `become/unbecome`。
- YAGNI 観点では、`japi` / `io` / `serialization` の JVM 依存 surface を actor モジュールで追いかける必要は薄い。typed 比較は `actor-typed` を対象に別レポートへ分けるのが妥当。


はい。現状は「untyped core の上に typed を載せている」が、typed が薄いラッパーだけではない状態です。

薄いラッパーに近い部分:

- actor_context.rs
  TypedActorContext は内部で untyped の [ActorContext] を包んでいます。
- typed_actor_adapter.rs
  typed actor を untyped の Actor として実行する adapter です。

typed 側だけで実装を持っている部分:

- behavior_runner.rs
  Behavior / BehaviorSignal / directive 遷移は typed 側のロジックです。
- typed_actor_adapter.rs
  message adapter registry、receive-timeout、dead letter 変換なども typed 側で持っています。
- core/typed 配下の receptionist、topic、group_router_builder、pool_router_builder、stash_buffer も typed 専用の実装です。

要するに、

- 実行基盤: untyped core
- 型付きの意味論: typed 側でかなり実装

です。
なので「classic 相当 core(untyped) をただラップして typed を作る」設計に寄せたいなら、まだ途中です。特に Behavior 系と adapter/timeout/router 周りは、いまは typed 側に実装が残っています。
