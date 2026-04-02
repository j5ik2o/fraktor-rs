# actor モジュール ギャップ分析

## 前提と集計範囲

- 比較対象:
  - fraktor-rs: `modules/actor/src/core/kernel`, `modules/actor/src/core/typed`, `modules/actor/src/std`
  - Pekko classic: `references/pekko/actor/src/main/scala/org/apache/pekko/actor`
  - Pekko typed: `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed`, `.../scaladsl`, `.../eventstream`, `.../receptionist`, `.../pubsub`, `.../delivery`
- `Pekko 公開型数` は companion object / Scala/Java DSL の重複を畳んだ「セマンティックな公開型 family」の集約計数とする。
- `fraktor-rs 公開型数` も同じ粒度で集計する。内部専用型、テスト専用型、同一責務の薄い別名は数えない。
- classic と typed の両方を対象にするが、JVM 専用 Java DSL や reflection/classloader 前提の契約は `n/a` ではなく今回の主集計から除外した。
- API ギャップが支配的かどうかを優先判定し、内部モジュール構造ギャップ分析はしきい値を満たした場合のみ行う。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 34 |
| fraktor-rs 公開型数 | 24（core: 21, std: 3） |
| カバレッジ（型単位） | 24/34 (71%) |
| ギャップ数 | 10（core: 7, std: 3） |

補足:

- typed 側は `Receptionist`、`Topic`、reliable delivery、typed `AskPattern`、typed `RouterBuilder` まで揃っており、主要 DSL はかなり進んでいる。
- 一方で classic 側は `ActorSelection`、public `actorOf` surface、classic `FSM`、Pekko IO family が未充足で、公開契約 parity の主ボトルネックになっている。
- そのため今回は API ギャップが支配的であり、構造比較は後続フェーズと判断する。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core / untyped kernel | 13 | 8 | 62% |
| core / typed ラッパー | 16 | 13 | 81% |
| std / アダプタ | 5 | 3 | 60% |

## カテゴリ別ギャップ

### classic コア契約 ✅ 実装済み 3/6 (50%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorSelection` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorSelection.scala:39` | 部分実装 | core/kernel | hard | fraktor-rs は `ActorSelectionResolver` による相対パス解決のみ (`modules/actor/src/core/kernel/actor/actor_selection/resolver.rs:15`) で、selection handle 本体の `tell` / `forward` / `resolveOne` / `toSerializationFormat` がない |
| `ActorRefFactory` public surface (`actorOf`, `stop`, `actorSelection`) | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorRefProvider.scala:189`, `:230`, `:255`, `:284` | 部分実装 | core/kernel | medium | fraktor-rs は `ActorContext::spawn_child` (`modules/actor/src/core/kernel/actor/actor_context.rs:218`) と `ExtendedActorSystem::spawn_system_actor` (`modules/actor/src/core/kernel/system/extended_actor_system.rs:148`) はあるが、classic `ActorSystem` 自体の public `actor_of`/`stop`/`actor_selection` 契約はない |
| `ActorRefProvider` full classic contract | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/ActorRefProvider.scala:47`, `:95`, `:100`, `:120`, `:145`, `:165` | 部分実装 | core/kernel | medium | fraktor-rs の trait は `supported_schemes` / `actor_ref` を中心に最小化されており (`modules/actor/src/core/kernel/actor/actor_ref_provider/base.rs:20`)、`deployer`, `tempContainer`, `unregisterTempActor`, external address 解決などの classic surface は未公開 |

### classic ライフサイクル / runtime ✅ 実装済み 4/7 (57%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| classic `FSM` / `AbstractFSM` / `LoggingFSM` | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FSM.scala:430`, `:937`, `references/pekko/actor/src/main/scala/org/apache/pekko/actor/AbstractFSM.scala:43` | 部分実装 | core/kernel | hard | fraktor-rs は typed 側に `FsmBuilder` (`modules/actor/src/core/typed/dsl/fsm_builder.rs:18`) を持つが、classic FSM の `when`, transition subscription, timeout, listener 契約はない |
| `Tcp` / `Udp` / `Dns` / `IO` family | Pekko actor moduleの public IO family 一式 | 未対応 | std | hard | fraktor-rs には `modules/actor/src/std/io/` のパッケージ境界だけがあり、`std.rs` から再公開もされていない (`modules/actor/src/std.rs:1`)。公開 API と実体の両方が不足している |
| `CoordinatedShutdown` advanced task helpers | `references/pekko/actor/src/main/scala/org/apache/pekko/actor/CoordinatedShutdown.scala:564`, `:648`, `:815`, `:897` | 部分実装 | std | medium | fraktor-rs は `add_task` / `timeout` / `run` まで (`modules/actor/src/std/system/coordinated_shutdown.rs:184`, `:209`, `:245`) で、`addCancellableTask`, `addActorTerminationTask`, JVM shutdown hook 系の helper がない |

### typed コア契約 ✅ 実装済み 8/10 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ActorSystem` が `ActorRef` / `RecipientRef` を兼ねる契約 | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorSystem.scala:45`, `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/ActorRef.scala:35`, `:126` | 部分実装 | core/typed | hard | fraktor-rs の `TypedActorSystem` は system handle であり (`modules/actor/src/core/typed/system.rs:161`)、`TypedActorRef` ではない。`user_guardian_ref()` はあるが、system 自体へ `tell` する契約は未提供 |
| typed `EventStream.Command` の型別 subscribe 契約 | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/eventstream/EventStream.scala:31`, `:52`, `:69` | 部分実装 | core/typed | medium | fraktor-rs の `EventStreamCommand` は `EventStreamEvent` 固定 publish と raw subscriber 登録のみ (`modules/actor/src/core/typed/eventstream/event_stream_command.rs:8`) で、Pekko の `ClassTag` ベース subtype subscription 契約がない |

### typed DSL / routing ✅ 実装済み 5/6 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GroupRouter` / `PoolRouter` fluent parity の未充足分 | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/scaladsl/Routers.scala:55`, `:90`, `:128`, `:173` | 部分実装 | core/typed | easy | fraktor-rs は builder 自体はかなり揃っており、`with_consistent_hash`, `with_routee_props`, `with_broadcast_predicate` まである (`modules/actor/src/core/typed/dsl/routing/pool_router_builder.rs:95`, `:137`, `:104`) が、group router の `preferLocalRoutees` 相当と Pekko の `Behavior` 兼 builder trait 契約はない |

### typed ディスカバリ / delivery ✅ 実装済み 4/5 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `TopicStats` の cluster-aware semantics | `references/pekko/actor-typed/src/main/scala/org/apache/pekko/actor/typed/pubsub/Topic.scala:120` | 部分実装 | core/typed | medium | fraktor-rs の `TopicStats` は `local_subscriber_count` と `topic_instance_count` を返すローカル集計 (`modules/actor/src/core/typed/pubsub/topic_stats.rs:5`) で、Pekko distributed pubsub 前提の cluster-wide semantics までは持たない |

## 内部モジュール構造ギャップ

今回は API ギャップが支配的なため省略する。

判定根拠:

- 型単位カバレッジが `71%` で、内部構造分析へ進む目安の `80%` を下回っている
- `hard` / `medium` の未実装ギャップが 8 件あり、しきい値 5 件を超えている
- 特に classic 側の `ActorSelection`、`actorOf` surface、`FSM`、`IO` family が未充足で、内部責務分割より公開契約 parity の穴埋めが先である

## 実装優先度

### Phase 1

- `GroupRouter` の `preferLocalRoutees` 相当を追加し、Pekko `Routers.group` builder の fluent parity を埋める（実装先層: `core/typed`）
- `CoordinatedShutdown` に `add_cancellable_task` 相当 helper を追加する（実装先層: `std`）
- `CoordinatedShutdown` に actor termination task helper を追加する（実装先層: `std`）

### Phase 2

- typed `EventStream.Command` に型別 subscribe/unsubscribe 契約を追加する（実装先層: `core/typed`）
- `TopicStats` の semantics を cluster-aware な契約へ拡張するか、少なくとも local-only との差を API で明示する（実装先層: `core/typed`）
- classic `ActorRefFactory` public surface を `ActorSystem` / `ExtendedActorSystem` 上へ追加する（実装先層: `core/kernel`）
- `ActorRefProvider` の classic contract を段階的に公開し、temp actor / address 解決 surface を埋める（実装先層: `core/kernel`）

### Phase 3

- `ActorSelection` handle 本体を実装し、`tell` / `forward` / `resolve_one` / `to_serialization_format` を揃える（実装先層: `core/kernel`）
- `TypedActorSystem` を `RecipientRef` 的に扱える契約へ寄せるか、同等 surface を別名で公開して parity を埋める（実装先層: `core/typed`）
- classic `FSM` / `AbstractFSM` / `LoggingFSM` family を実装する（実装先層: `core/kernel`）
- `Tcp` / `Udp` / `Dns` / `IO` family を public API と実装の両方で追加する（実装先層: `std`）

### 対象外（n/a）

- なし

補足:

- Java 継承 DSL や reflection/classloader 前提の型は今回の主集計から除外しており、ここへは載せていない
- したがって、この優先度表は「actor parity を埋める実装順」であり、JVM 固有 API の移植順ではない

## まとめ

- 全体評価: typed 側の主要機能はかなりカバー済みだが、classic parity の基盤部分がまだ手薄
- 低コストで前進できる項目: group router fluent parity、`CoordinatedShutdown` helper surface、typed event stream の型別 subscribe 契約
- 主要ギャップ: `ActorSelection`、classic `actorOf` surface、classic `FSM`、Pekko IO family、`TypedActorSystem` の recipient 契約
- 次のボトルネック評価: API ギャップがまだ支配的であり、内部構造ではなく公開契約 parity の継続解消が先行課題である
