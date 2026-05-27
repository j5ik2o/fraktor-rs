# persistence モジュール ギャップ分析

更新日: 2026-05-28 JST

## 比較スコープ定義

この分析は Apache Pekko persistence の raw API 数をそのまま移植対象にしない。fraktor-rs の persistence では write-side persistence runtime を対象にし、`persistence-query`、testkit / TCK、Java / Scala DSL convenience、JVM 固有の plugin loading は parity 分母から除外する。

現行 fraktor-rs の persistence は、旧スコープに残っている `modules/persistence-core/src/core/` ではなく、`modules/persistence-core-kernel/src/` と `modules/persistence-core-typed/src/` に分割済みである。`persistence-adaptor-std` は存在しないため、ファイル IO や std runtime adapter は未実装ギャップとして扱うが、adaptor crate が未作成であること自体は減点しない。

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| classic persistent actor | `modules/persistence-core-kernel/src/persistent/` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/PersistentActor.scala`, `Eventsourced.scala` |
| recovery / journal / snapshot | `modules/persistence-core-kernel/src/journal/`, `modules/persistence-core-kernel/src/snapshot/`, `modules/persistence-core-kernel/src/persistent/recovery.rs` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/`, `snapshot/`, `JournalProtocol.scala`, `SnapshotProtocol.scala` |
| persistent representation / adapters / serialization | `modules/persistence-core-kernel/src/persistent/persistent_repr.rs`, `modules/persistence-core-kernel/src/journal/`, `modules/persistence-core-kernel/src/serialization/` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Persistent.scala`, `journal/EventAdapter.scala`, `serialization/` |
| durable state store contract | `modules/persistence-core-kernel/src/state/` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/state/` |
| delivery / FSM compatibility | `modules/persistence-core-kernel/src/delivery/`, `modules/persistence-core-kernel/src/fsm/` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/AtLeastOnceDelivery.scala`, `fsm/PersistentFSM.scala` |
| plugin / extension / in-memory stores | `modules/persistence-core-kernel/src/extension/`, `modules/persistence-core-kernel/src/journal/persistence_plugin_proxy.rs`, `modules/persistence-core-kernel/src/journal/in_memory_journal.rs`, `modules/persistence-core-kernel/src/snapshot/in_memory_snapshot_store.rs` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Persistence.scala`, `journal/PersistencePluginProxy.scala`, `journal/inmem/InmemJournal.scala` |
| typed write-side API | `modules/persistence-core-typed/src/` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/` |

### 対象外

| 除外項目 | 理由 |
|----------|------|
| `persistence-query` | write-side runtime とは別スコープ。ユーザーが query 調査を明示した場合だけ対象 |
| `persistence-testkit`, `persistence-tck`, `persistence-typed-tests` | runtime API ではない |
| `persistence-shared` の `src/test` 配下 | 現在の参照ツリーでは main runtime API がなく、shared LevelDB / serializer spec は test scope |
| JDBC / Cassandra / LevelDB など特定 storage plugin 完全互換 | storage backend 実装技術ごとの互換は別スコープ |
| Java DSL wrapper / `javadsl` package | Rust API として再現不要 |
| Scala implicit / package object / syntax sugar | Rust API として再現不要 |
| HOCON plugin loading / JVM reflection / classloader | JVM 固有 |
| replicated event sourcing / CRDT / typed reliable delivery queue | `persistence-typed` 内にあるが、現 persistence 固定スコープの列挙対象外。必要なら replication / delivery として別調査 |

Pekko 側 raw 抽出は `references/pekko/persistence/src/main/scala` と `references/pekko/persistence-typed/src/main/scala` を対象にした。raw public type declarations は 352 件、主要 `def` declarations は 1405 件である。これには private / internal / Java DSL / JVM 固有 / scope 外の replication 系 API が含まれるため、parity カバレッジ分母には使わない。

fraktor-rs 側 raw 抽出は `modules/persistence-core-kernel/src/` と `modules/persistence-core-typed/src/` から `*_test.rs` / `lib_test.rs` を除外して実施した。raw public type declarations は 88 件（kernel: 66、typed: 22）、raw public method declarations は 321 件（kernel: 242、typed: 79）である。このうち `pub(crate)` を除いた外部到達可能な `pub` type declarations は 79 件（kernel: 62、typed: 17）、`pub` method declarations は 282 件（kernel: 214、typed: 68）である。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 80 |
| fraktor-rs 固定スコープ対応概念 | 63 |
| 固定スコープ概念カバレッジ | 63/80 (79%) |
| raw public type declarations | 88（kernel: 66, typed: 22） |
| raw public method declarations | 321（kernel: 242, typed: 79） |
| externally reachable `pub` type declarations | 79（kernel: 62, typed: 17） |
| externally reachable `pub` method declarations | 282（kernel: 214, typed: 68） |
| hard / medium / easy / trivial gap | 4 / 6 / 0 / 0 |
| panic 系スタブ | 0 件 |
| 機能 placeholder / TODO | 0 件 |

raw declaration count は参考値であり、parity 分母に使わない。

classic write-side persistence は、persistent actor、journal、snapshot、event adapter、at-least-once delivery、durable state store の基本契約が揃っている。typed write-side は Pekko typed `EventSourcedBehavior` / `Effect` の直移植ではなく、通常の typed `Behavior` を保ったまま hidden child store actor に永続化を委譲する effector-first API として実装されている。

2026-05-28 の再検証では、現行 crate 境界が `persistence-core-kernel` / `persistence-core-typed` であり、`persistence-adaptor-std` は存在しないことを確認した。Pekko 側 raw extraction と fraktor-rs 側 raw / externally reachable extraction の件数は 2026-05-27 版から変化していない。前回「部分実装」として残っていた adapter manifest と serializer manifest の接続は、`PersistenceContext::to_journal_repr`、`PersistentRepr::adapter_type_id`、`MessageSerializer` の wire encoding で接続済みと判定する。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / kernel | `PersistentActor`, `Eventsourced`, `Recovery`, journal, snapshot, adapter, delivery, durable state store | `Eventsourced`, `PersistentActor`, `Journal`, `AtomicWrite`, `SnapshotStore`, `EventAdapters`, `AtLeastOnceDelivery`, `DurableStateStore` が存在 | classic write-side の主要契約は中程度以上に対応。std local snapshot store と plugin proxy runtime semantics が不足 |
| core / typed | `EventSourcedBehavior`, `Effect`, signal, typed recovery / retention, `DurableStateBehavior` | `PersistenceEffector`, `PersistenceEffectorConfig`, `PersistenceEffectorSignal`, `PersistenceMode`, `SnapshotCriteria`, `RetentionCriteria`, `BackoffConfig`, typed `Recovery`, typed `EventAdapter`, `SnapshotAdapter`, `DurableStateSignal` を提供 | effector-first API は進んでいるが、Pekko 互換の behavior DSL と typed durable state behavior は未実装 |
| std / adaptor | local snapshot store、runtime plugin adapter | 対応 crate なし。in-memory store は kernel に存在 | ファイル IO / runtime adapter は未対応 |

## カテゴリ別ギャップ

ギャップ（未対応・部分実装・n/a）のみテーブルに列挙する。実装済みはカテゴリの件数カウントに含めるが、テーブル行には追加しない。

### 1. Persistent actor / recovery / lifecycle ✅ 実装済み 11/15 (73%)

fraktor-rs は Pekko の `PersistentActor` と複数 mix-in trait を、`Eventsourced` と `PersistentActor` に統合している。`persist` / `persist_async` / `persist_all` / `defer` / snapshot / delete / recovery callbacks は存在する。根拠は `modules/persistence-core-kernel/src/persistent/persistent_actor.rs:23`、`modules/persistence-core-kernel/src/persistent/eventsourced.rs:19`、`modules/persistence-core-kernel/src/persistent/eventsourced.rs:52`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RecoveryCompleted` classic public signal type | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/PersistentActor.scala:29` | 実装済み / non-goal | core | n/a | classic は `on_recovery_completed` callback と internal `JournalResponseAction::RecoveryCompleted` で表現済み。typed は `PersistenceEffectorSignal::RecoveryCompleted` を公開済み |
| HOCON `StashOverflowStrategyConfigurator` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/PersistentActor.scala:160` | 対象外 | core | n/a | JVM / HOCON configurator facade。Rust 側は `PersistentActor::stash_overflow_strategy` と `stash_capacity` で方針を直接返す |

### 2. Journal / snapshot store protocol ✅ 実装済み 13/16 (81%)

`Journal` は Pekko の `AsyncWriteJournal` と `AsyncRecovery` を no_std GAT future で統合した trait として存在し、`JournalActor` / `JournalMessage` / `JournalResponse` もある。`SnapshotStore`、`SnapshotActor`、`SnapshotMessage`、`SnapshotResponse`、`SnapshotMetadata`、`SnapshotSelectionCriteria` も実装済み。根拠は `modules/persistence-core-kernel/src/journal/base.rs:12`、`modules/persistence-core-kernel/src/snapshot/snapshot_store.rs:10`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `LocalSnapshotStore` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/snapshot/local/LocalSnapshotStore.scala:40` | 未対応 | std/snapshot | medium | ファイルシステム依存。core ではなく将来の `persistence-adaptor-std` が妥当 |
| `receivePluginInternal` for advanced journal / snapshot plugins | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/AsyncWriteJournal.scala:311`, `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/snapshot/local/LocalSnapshotStore.scala:100` | 部分実装 | core/plugin + std/runtime | medium | Rust 側の `Journal` / `SnapshotStore` trait は storage contract を持つが、plugin actor が追加内部メッセージを扱う拡張口はない |

### 3. Persistent representation / adapters / serialization ✅ 実装済み 14/14 (100%)

`PersistentRepr` は persistence id、sequence number、manifest、writer uuid、timestamp、deleted、sender、metadata、adapter resolution key を保持する。`WriteEventAdapter`、`ReadEventAdapter`、`IdentityEventAdapter`、`EventSeq`、`EventAdapters`、`Tagged` も存在する。`MessageSerializer` は `PersistentRepr` / `AtomicWrite` を actor serialization registry 経由で payload / metadata へ委譲し、manifest と adapter type binding を wire に保持する。根拠は `modules/persistence-core-kernel/src/persistent/persistent_repr.rs:20`、`modules/persistence-core-kernel/src/journal/event_adapters.rs:20`、`modules/persistence-core-kernel/src/serialization/message_serializer.rs:45`、`modules/persistence-core-kernel/src/serialization/message_serializer.rs:60`、`modules/persistence-core-kernel/src/serialization/message_serializer.rs:64`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| Pekko protobuf byte compatibility | `references/pekko/persistence/src/main/protobuf/MessageFormats.proto` | 対象外 | core/serialization | n/a | fraktor-rs は actor serialization registry への委譲で同等契約を満たす。Pekko protobuf の byte-level 互換は non-goal |

### 4. At-least-once delivery / unconfirmed delivery ✅ 実装済み 6/7 (86%)

`AtLeastOnceDelivery`、`AtLeastOnceDeliveryConfig`、`AtLeastOnceDeliverySnapshot`、`UnconfirmedDelivery`、`UnconfirmedWarning`、`RedeliveryTick` は存在する。未確認配送の snapshot / restore、redelivery、confirm も実装済み。根拠は `modules/persistence-core-kernel/src/delivery/at_least_once_delivery.rs:21`、`modules/persistence-core-kernel/src/delivery/at_least_once_delivery.rs:103`、`modules/persistence-core-kernel/src/delivery/at_least_once_delivery.rs:204`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| Java abstract wrapper | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/AtLeastOnceDelivery.scala:426` | 対象外 | core | n/a | Java interop 専用 |

### 5. Durable State store contract ✅ 実装済み 6/8 (75%)

`DurableStateStore`、`DurableStateUpdateStore`、`DurableStateStoreProvider`、`DurableStateStoreRegistry`、`DurableStateError` は存在する。`DurableStateStore` は expected revision を受け取り、`DurableStateUpdateStore::changes` は tag と offset から `DurableStateChange` を返す。typed `DurableStateBehavior` との実行統合は別カテゴリの未達として残る。根拠は `modules/persistence-core-kernel/src/state/durable_state_store.rs:12`、`modules/persistence-core-kernel/src/state/durable_state_update_store.rs:6`、`modules/persistence-core-kernel/src/state/durable_state_change.rs:11`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| HOCON provider loading | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/state/DurableStateStoreProvider.scala:24` | 対象外 | std/runtime | n/a | Rust 側は provider / store trait を直接登録するモデル。HOCON loading は JVM 固有 |

### 6. Plugin / extension / in-memory stores ✅ 実装済み 5/7 (71%)

`PersistenceExtension`、`PersistenceExtensionId`、`PersistenceExtensionInstaller`、`PersistencePluginProxy`、`InMemoryJournal`、`InMemorySnapshotStore` は存在する。HOCON loading と runtime plugin id selection は対象外であり、Rust 側は `Journal` / `SnapshotStore` trait 実装を注入するモデルを採用する。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| plugin target location / proxy extension semantics | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/PersistencePluginProxy.scala:38`, `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/PersistencePluginProxy.scala:85` | 部分実装 | core/plugin + std/runtime | medium | `PersistencePluginProxy<J, S>` は forwarding object だが、Pekko の target location / extension actor semantics まではない |
| `RuntimePluginConfig` / config based plugin selection | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Persistence.scala:40` | 対象外 | core/config | n/a | trait 実装注入モデルでは HOCON plugin selection を再現しない |

### 7. Persistent FSM compatibility ✅ 実装済み 1/1 (100%)

Pekko の `PersistentFSM` family は deprecated だが、固定スコープでは compatibility marker として確認した。fraktor-rs には最小契約の `PersistentFsm` trait が存在し、state transition event の persist / apply を `PersistentActor` 上で表現できる。根拠は `modules/persistence-core-kernel/src/fsm/persistent_fsm.rs:17`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| full FSM DSL / migration helper | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/fsm/PersistentFSM.scala` | 対象外 | core | n/a | Pekko 側でも legacy / deprecated。fraktor-rs は最小 `PersistentFsm` 契約を持つ |

### 8. Typed write-side effector / EventSourcedBehavior / signal ✅ 実装済みまたは代替 5/9 (56%)

Pekko persistence の現行推奨 write-side API は typed `EventSourcedBehavior` と `Effect` 体系である。一方、fraktor-rs は専用 DSL ではなく、`PersistenceEffector` で通常の typed `Behavior` に永続化 side effect を注入する。`PersistenceId`、effector 経由の persist/snapshot 操作、`RetentionCriteria`、typed recovery selection、typed event/snapshot adapter contract は実装済みまたは明確な代替とみなせる。Pekko 互換 DSL としての `EventSourcedBehavior` / `EffectBuilder`、published event、behavior-level persist failure supervision は未達である。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `EventSourcedBehavior[C,E,S]` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/EventSourcedBehavior.scala:36`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/EventSourcedBehavior.scala:138` | 方針差あり | core/typed | hard | 専用 DSL は導入せず、`modules/persistence-core-typed/src/persistence_effector.rs:52` の `PersistenceEffector::props(config, on_ready)` で通常 `Behavior` と統合 |
| `Effect` / `EffectBuilder` / `ReplyEffect` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/Effect.scala:132`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/Effect.scala:144`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/Effect.scala:196` | effector API で一部代替 | core/typed | hard | `persist_event` / `persist_events` / `persist_snapshot` はあるが、reply/stash/unhandled/stop の effect model はない |
| `EventSourcedSignal` family | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/EventSourcedSignal.scala:27`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/EventSourcedSignal.scala:58`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/EventSourcedSignal.scala:131` | 部分実装 | core/typed | medium | `modules/persistence-core-typed/src/persistence_effector_signal.rs:11` は recovery / persisted / snapshot / delete / failure を private message 経由で表すが、Pekko の公開 signal family と一致しない |
| `PublishedEvent` / `EventRejectedException` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/PublishedEvent.scala:28`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/EventRejectedException.scala:19` | 未対応 | core/typed | medium | event publication と rejection signal / error の公開契約がない |
| behavior-level `onPersistFailure` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/EventSourcedBehavior.scala:230` | 部分実装 | core/typed | medium | `BackoffConfig` と hidden store actor の `BackoffSupervisor` wiring はあるが、Pekko の behavior-level supervision hook としては未統合 |

### 9. Typed DurableStateBehavior ✅ 実装済み 1/3 (33%)

Durable state store contract は kernel に存在するが、Pekko typed の write-side behavior API は未実装である。`DurableStateSignal` は先行して公開されている。根拠は `modules/persistence-core-typed/src/durable_state_signal.rs:13`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `DurableStateBehavior[C,S]` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/DurableStateBehavior.scala:36`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/DurableStateBehavior.scala:127` | 未対応 | core/typed | hard | typed `Behavior` と durable state store の統合が必要。`withTag` / `onPersistFailure` / `snapshotAdapter` も含む |
| durable state `Effect` / `EffectBuilder` / `ReplyEffect` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/Effect.scala:124`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/Effect.scala:136`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/Effect.scala:188` | 未対応 | core/typed | hard | persist / delete / none / unhandled / stop / stash / reply の effect model が必要 |

## 内部モジュール構造ギャップ

今回は API ギャップが支配的なため省略する。固定スコープ概念カバレッジは 63/80 (79%) で 80% に届かず、hard / medium の未実装ギャップも 10 件ある。特に typed `EventSourcedBehavior` direct DSL、typed `DurableStateBehavior`、std local snapshot store、plugin runtime extension semantics が未達である。責務分割の細部比較より先に、behavior DSL と std adapter contract の境界を決める段階である。

次版で構造分析へ進む場合の観点は以下になる。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| classic と typed の境界 | `persistence-core-kernel` が classic runtime、`persistence-core-typed` が effector-first typed API を担当 | typed `DurableStateBehavior` を同じ typed crate に追加するか、別 change に分けるか |
| journal / serializer の境界 | `Journal` は `AtomicWrite` を受け、persistence serializers は actor-core serialization registry に contributor として登録される | std local snapshot store がこの contract をどう呼び出すか |
| durable state revision model | store trait は expected revision と optional tag を受け、tagged update metadata を返す | typed DurableStateBehavior がこの contract をどう実行するか |
| plugin adapter 境界 | core extension は generic journal / snapshot を直接受ける | std runtime で plugin target location / local snapshot store をどう表すか |
| typed effector と Pekko typed DSL の境界 | `PersistenceEffector` は通常 `Behavior` に統合されるが、Pekko の `EffectBuilder` / signal / adapter をそのまま露出しない | parity 目標を effector-first で固定するか、Pekko direct DSL を追加するか |

## 実装優先度

ここで出す優先度は「今の要求で実装すべきか」ではなく、「Pekko parity ギャップをどの順で埋めるか」を示す。YAGNI は適用しない。以下は直前のカテゴリ別ギャップに列挙済みの項目だけを再配置する。

### Phase 1: trivial / easy

現時点で未実装の trivial / easy gap はない。

### Phase 2: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `LocalSnapshotStore` | std/snapshot | カテゴリ2 |
| `receivePluginInternal` for advanced journal / snapshot plugins | core/plugin + std/runtime | カテゴリ2 |
| plugin target location / proxy extension semantics | core/plugin + std/runtime | カテゴリ6 |
| `EventSourcedSignal` family | core/typed | カテゴリ8 |
| `PublishedEvent` / `EventRejectedException` | core/typed | カテゴリ8 |
| behavior-level `onPersistFailure` | core/typed | カテゴリ8 |

### Phase 3: hard

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `EventSourcedBehavior[C,E,S]` | core/typed | カテゴリ8 |
| `Effect` / `EffectBuilder` / `ReplyEffect` | core/typed | カテゴリ8 |
| `DurableStateBehavior[C,S]` | core/typed | カテゴリ9 |
| durable state `Effect` / `EffectBuilder` / `ReplyEffect` | core/typed | カテゴリ9 |

### 対象外 (n/a)

| 項目 | 理由 |
|------|------|
| `persistence-query` | write-side runtime とは別スコープ |
| Java DSL wrapper / `javadsl/*` | Rust API として再現不要 |
| Scala syntax sugar / implicit ops | Rust API として再現不要 |
| HOCON dynamic loading / JVM reflection / classloader | JVM 固有 |
| `persistence-testkit`, `persistence-tck`, typed tests | runtime API ではない |
| JDBC / Cassandra / LevelDB plugin 完全互換 | storage backend 実装技術ごとの互換は別スコープ |
| full `PersistentFSM` DSL / migration helper | Pekko 側で legacy / deprecated。fraktor-rs は最小 `PersistentFsm` 契約を持つ |
| replicated event sourcing / CRDT / typed reliable delivery queue | 現 persistence 固定スコープ外。必要なら別スコープとして調査 |

## まとめ

persistence は classic write-side の基礎部品がかなり揃っている。`PersistentActor`、journal、snapshot、event adapter、serializer、at-least-once delivery、durable state store registry、in-memory store は存在し、panic 系スタブも見つからない。今回の再検証で adapter manifest と serializer manifest の接続は実装済みに更新した。

parity を低コストで前進させる未実装機能は、`LocalSnapshotStore`、plugin target location / proxy extension semantics、typed `EventSourcedSignal` family、`PublishedEvent` / `EventRejectedException`、behavior-level `onPersistFailure` である。いずれも Phase 2 に置けるが、std adapter crate がないため `LocalSnapshotStore` は crate 境界設計も同時に必要になる。

parity 上の主要ギャップは、typed `EventSourcedBehavior` / `EffectBuilder` と typed `DurableStateBehavior` / durable state `EffectBuilder` である。typed write-side は effector-first API として前進しているが、Pekko parity の観点では behavior-level DSL と durable state behavior execution がまだ閉じていない。API ギャップが支配的なため、内部構造比較は後続フェーズで扱うのが妥当である。
