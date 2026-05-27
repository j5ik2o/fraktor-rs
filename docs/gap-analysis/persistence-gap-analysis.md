# persistence モジュール ギャップ分析

更新日: 2026-05-28 JST

## 比較スコープ定義

この分析は Apache Pekko persistence の raw API 数をそのまま移植対象にしない。fraktor-rs の persistence では write-side persistence runtime を対象にし、`persistence-query`、testkit / TCK、Java / Scala DSL convenience、JVM 固有の plugin loading は parity 分母から除外する。

現行 fraktor-rs の persistence は、旧スコープに残っている `modules/persistence-core/src/core/` ではなく、`modules/persistence-core-kernel/src/`、`modules/persistence-core-typed/src/`、`modules/persistence-adaptor-std/src/` に分割済みである。`persistence-adaptor-std` はファイル IO を伴う local snapshot store を担当し、core 側の port / policy から std adapter を差し込む依存方向を維持している。

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| classic persistent actor | `modules/persistence-core-kernel/src/persistent/` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/PersistentActor.scala`, `Eventsourced.scala` |
| recovery / journal / snapshot | `modules/persistence-core-kernel/src/journal/`, `modules/persistence-core-kernel/src/snapshot/`, `modules/persistence-core-kernel/src/persistent/recovery.rs` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/`, `snapshot/`, `JournalProtocol.scala`, `SnapshotProtocol.scala` |
| persistent representation / adapters / serialization | `modules/persistence-core-kernel/src/persistent/persistent_repr.rs`, `modules/persistence-core-kernel/src/journal/`, `modules/persistence-core-kernel/src/serialization/` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Persistent.scala`, `journal/EventAdapter.scala`, `serialization/` |
| durable state store contract | `modules/persistence-core-kernel/src/state/` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/state/` |
| delivery / FSM compatibility | `modules/persistence-core-kernel/src/delivery/`, `modules/persistence-core-kernel/src/fsm/` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/AtLeastOnceDelivery.scala`, `fsm/PersistentFSM.scala` |
| plugin / extension / in-memory stores | `modules/persistence-core-kernel/src/extension/`, `modules/persistence-core-kernel/src/journal/persistence_plugin_proxy.rs`, `modules/persistence-core-kernel/src/journal/in_memory_journal.rs`, `modules/persistence-core-kernel/src/snapshot/in_memory_snapshot_store.rs` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Persistence.scala`, `journal/PersistencePluginProxy.scala`, `journal/inmem/InmemJournal.scala` |
| std snapshot adapter | `modules/persistence-adaptor-std/src/snapshot/` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/snapshot/local/LocalSnapshotStore.scala` |
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

fraktor-rs 側 raw 抽出は `modules/persistence-core-kernel/src/`、`modules/persistence-core-typed/src/`、`modules/persistence-adaptor-std/src/` から `*_test.rs` / `lib_test.rs` を除外して実施した。raw public type declarations は 100 件（kernel: 73、typed: 25、std: 2）、raw public method declarations は 359 件（kernel: 252、typed: 99、std: 8）である。このうち `pub(crate)` を除いた外部到達可能な `pub` type declarations は 91 件（kernel: 69、typed: 20、std: 2）、`pub` method declarations は 318 件（kernel: 224、typed: 87、std: 7）である。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 80 |
| fraktor-rs 固定スコープ対応概念 | 69 |
| 固定スコープ概念カバレッジ | 69/80 (86%) |
| raw public type declarations | 100（kernel: 73, typed: 25, std: 2） |
| raw public method declarations | 359（kernel: 252, typed: 99, std: 8） |
| externally reachable `pub` type declarations | 91（kernel: 69, typed: 20, std: 2） |
| externally reachable `pub` method declarations | 318（kernel: 224, typed: 87, std: 7） |
| hard / medium / easy / trivial gap | 4 / 0 / 0 / 0 |
| panic 系スタブ | 0 件 |
| 機能 placeholder / TODO | 0 件 |

raw declaration count は参考値であり、parity 分母に使わない。

classic write-side persistence は、persistent actor、journal、snapshot、event adapter、at-least-once delivery、durable state store の基本契約が揃っている。typed write-side は Pekko typed `EventSourcedBehavior` / `Effect` の直移植ではなく、通常の typed `Behavior` を保ったまま hidden child store actor に永続化を委譲する effector-first API として実装されている。

2026-05-28 の再検証では、`origin/main` 取り込み後の現行 crate 境界が `persistence-core-kernel` / `persistence-core-typed` / `persistence-adaptor-std` であることを確認した。Pekko 側 raw extraction は 2026-05-27 版から変化していないが、fraktor-rs 側は std local snapshot store、plugin proxy extension、typed event-sourced signal、published event / rejection error、persist failure backoff hook が追加された。前回「部分実装」として残っていた adapter manifest と serializer manifest の接続は、`PersistenceContext::to_journal_repr`、`PersistentRepr::adapter_type_id`、`MessageSerializer` の wire encoding で接続済みと判定する。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / kernel | `PersistentActor`, `Eventsourced`, `Recovery`, journal, snapshot, adapter, delivery, durable state store | `Eventsourced`, `PersistentActor`, `Journal`, `AtomicWrite`, `SnapshotStore`, `EventAdapters`, `AtLeastOnceDelivery`, `DurableStateStore`, plugin message handler, proxy actor / extension が存在 | classic write-side の主要契約は中程度以上に対応。plugin proxy runtime semantics も core 側に入った |
| core / typed | `EventSourcedBehavior`, `Effect`, signal, typed recovery / retention, `DurableStateBehavior` | `PersistenceEffector`, `PersistenceEffectorConfig`, `PersistenceEffectorSignal`, `EventSourcedSignal`, `PublishedEvent`, `EventRejectedError`, `PersistenceMode`, `SnapshotCriteria`, `RetentionCriteria`, `BackoffConfig`, typed `Recovery`, typed `EventAdapter`, `SnapshotAdapter`, `DurableStateSignal` を提供 | effector-first API は進んでいるが、Pekko 互換の behavior DSL と typed durable state behavior は未実装 |
| std / adaptor | local snapshot store、runtime plugin adapter | `persistence-adaptor-std` が存在し、filesystem-backed `LocalSnapshotStore` を提供。plugin adapter の runtime target は core extension で扱う | local snapshot store は対応済み。storage plugin 完全互換は対象外 |

## カテゴリ別ギャップ

ギャップ（未対応・部分実装・n/a）のみテーブルに列挙する。実装済みはカテゴリの件数カウントに含めるが、テーブル行には追加しない。

### 1. Persistent actor / recovery / lifecycle ✅ 実装済み 11/15 (73%)

fraktor-rs は Pekko の `PersistentActor` と複数 mix-in trait を、`Eventsourced` と `PersistentActor` に統合している。`persist` / `persist_async` / `persist_all` / `defer` / snapshot / delete / recovery callbacks は存在する。根拠は `modules/persistence-core-kernel/src/persistent/persistent_actor.rs:23`、`modules/persistence-core-kernel/src/persistent/eventsourced.rs:19`、`modules/persistence-core-kernel/src/persistent/eventsourced.rs:52`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RecoveryCompleted` classic public signal type | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/PersistentActor.scala:29` | 実装済み / non-goal | core | n/a | classic は `on_recovery_completed` callback と internal `JournalResponseAction::RecoveryCompleted` で表現済み。typed は `PersistenceEffectorSignal::RecoveryCompleted` を公開済み |
| HOCON `StashOverflowStrategyConfigurator` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/PersistentActor.scala:160` | 対象外 | core | n/a | JVM / HOCON configurator facade。Rust 側は `PersistentActor::stash_overflow_strategy` と `stash_capacity` で方針を直接返す |

### 2. Journal / snapshot store protocol ✅ 実装済み 16/16 (100%)

`Journal` は Pekko の `AsyncWriteJournal` と `AsyncRecovery` を no_std GAT future で統合した trait として存在し、`JournalActor` / `JournalMessage` / `JournalResponse` もある。`SnapshotStore`、`SnapshotActor`、`SnapshotMessage`、`SnapshotResponse`、`SnapshotMetadata`、`SnapshotSelectionCriteria` も実装済み。`LocalSnapshotStore` は std adapter に入り、advanced plugin message hook も journal / snapshot に分離された。根拠は `modules/persistence-core-kernel/src/journal/base.rs:12`、`modules/persistence-core-kernel/src/snapshot/snapshot_store.rs:10`、`modules/persistence-adaptor-std/src/snapshot/local_snapshot_store.rs:72`、`modules/persistence-adaptor-std/src/snapshot/local_snapshot_store.rs:489`、`modules/persistence-core-kernel/src/journal/journal_plugin_message_handler.rs:8`、`modules/persistence-core-kernel/src/snapshot/snapshot_plugin_message_handler.rs:8`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| なし | - | - | - | - | 固定スコープ上の未対応 gap はなし |

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

### 6. Plugin / extension / in-memory stores ✅ 実装済み 7/7 (100%)

`PersistenceExtension`、`PersistenceExtensionId`、`PersistenceExtensionInstaller`、`PersistencePluginProxy`、`PersistencePluginProxyActor`、`PersistencePluginProxyExtensionInstaller`、`InMemoryJournal`、`InMemorySnapshotStore` は存在する。HOCON loading と runtime plugin id selection は対象外であり、Rust 側は `Journal` / `SnapshotStore` trait 実装または proxy extension を注入するモデルを採用する。根拠は `modules/persistence-core-kernel/src/journal/persistence_plugin_proxy_actor.rs:23`、`modules/persistence-core-kernel/src/journal/persistence_plugin_proxy_actor.rs:47`、`modules/persistence-core-kernel/src/extension/persistence_plugin_proxy_extension_installer.rs:20`、`modules/persistence-core-kernel/src/extension/persistence_plugin_proxy_extension_installer.rs:45`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RuntimePluginConfig` / config based plugin selection | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Persistence.scala:40` | 対象外 | core/config | n/a | trait 実装注入モデルでは HOCON plugin selection を再現しない |

### 7. Persistent FSM compatibility ✅ 実装済み 1/1 (100%)

Pekko の `PersistentFSM` family は deprecated だが、固定スコープでは compatibility marker として確認した。fraktor-rs には最小契約の `PersistentFsm` trait が存在し、state transition event の persist / apply を `PersistentActor` 上で表現できる。根拠は `modules/persistence-core-kernel/src/fsm/persistent_fsm.rs:17`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| full FSM DSL / migration helper | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/fsm/PersistentFSM.scala` | 対象外 | core | n/a | Pekko 側でも legacy / deprecated。fraktor-rs は最小 `PersistentFsm` 契約を持つ |

### 8. Typed write-side effector / EventSourcedBehavior / signal ✅ 実装済みまたは代替 7/9 (78%)

Pekko persistence の現行推奨 write-side API は typed `EventSourcedBehavior` と `Effect` 体系である。一方、fraktor-rs は専用 DSL ではなく、`PersistenceEffector` で通常の typed `Behavior` に永続化 side effect を注入する。`PersistenceId`、effector 経由の persist/snapshot 操作、`RetentionCriteria`、typed recovery selection、typed event/snapshot adapter contract、event-sourced signal、published event、persist failure backoff hook は実装済みまたは明確な代替とみなせる。Pekko 互換 DSL としての `EventSourcedBehavior` / `EffectBuilder` は未達である。根拠は `modules/persistence-core-typed/src/event_sourced_signal.rs:16`、`modules/persistence-core-typed/src/published_event.rs:13`、`modules/persistence-core-typed/src/event_rejected_error.rs:15`、`modules/persistence-core-typed/src/persistence_effector_config.rs:190`、`modules/persistence-core-typed/src/persistence_effector.rs:709`、`modules/persistence-core-typed/src/persistence_effector.rs:732`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `EventSourcedBehavior[C,E,S]` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/EventSourcedBehavior.scala:36`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/EventSourcedBehavior.scala:138` | 方針差あり | core/typed | hard | 専用 DSL は導入せず、`modules/persistence-core-typed/src/persistence_effector.rs:52` の `PersistenceEffector::props(config, on_ready)` で通常 `Behavior` と統合 |
| `Effect` / `EffectBuilder` / `ReplyEffect` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/Effect.scala:132`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/Effect.scala:144`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/Effect.scala:196` | effector API で一部代替 | core/typed | hard | `persist_event` / `persist_events` / `persist_snapshot` はあるが、reply/stash/unhandled/stop の effect model はない |

### 9. Typed DurableStateBehavior ✅ 実装済み 1/3 (33%)

Durable state store contract は kernel に存在するが、Pekko typed の write-side behavior API は未実装である。`DurableStateSignal` は先行して公開されている。根拠は `modules/persistence-core-typed/src/durable_state_signal.rs:13`。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `DurableStateBehavior[C,S]` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/DurableStateBehavior.scala:36`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/DurableStateBehavior.scala:127` | 未対応 | core/typed | hard | typed `Behavior` と durable state store の統合が必要。`withTag` / `onPersistFailure` / `snapshotAdapter` も含む |
| durable state `Effect` / `EffectBuilder` / `ReplyEffect` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/Effect.scala:124`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/Effect.scala:136`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/Effect.scala:188` | 未対応 | core/typed | hard | persist / delete / none / unhandled / stop / stash / reply の effect model が必要 |

## 内部モジュール構造ギャップ

固定スコープ概念カバレッジは 69/80 (86%) で 80% を超え、medium gap も 0 件になったため、構造ギャップも確認する。現時点の構造上の主論点は、classic / std 側ではなく typed API 側の behavior DSL / effect interpreter の有無である。

| ギャップ名 | Pekko側の根拠 | fraktor-rs側の現状 | 問題の種類 | 推奨アクション | 緊急度 |
|------------|----------------|--------------------|------------|----------------|--------|
| event-sourced behavior DSL / effect interpreter の集約点不足 | `persistence-typed/.../scaladsl/EventSourcedBehavior.scala`, `Effect.scala` | `PersistenceEffector` が通常 `Behavior` に side effect を注入する。`Effect` / `ReplyEffect` / stash / stop / unhandled を統一する dedicated interpreter はない | 未分離 / 方針差 | effector-first 方針を正式採用するなら non-goal と明記。Pekko direct parity を狙うなら `event_sourced_behavior` / `effect` 相当の typed submodule を追加する | high |
| durable state behavior runtime の配置先未定 | `persistence-typed/.../state/scaladsl/DurableStateBehavior.scala`, `state/scaladsl/Effect.scala` | kernel の `DurableStateStore` と typed `DurableStateSignal` はあるが、typed `Behavior` と durable state store を接続する runtime / effect model はない | 未配置 | `persistence-core-typed` 内に durable state behavior 用 submodule を追加し、kernel store trait への adapter を集約する | high |

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| classic と typed の境界 | `persistence-core-kernel` が classic runtime、`persistence-core-typed` が effector-first typed API を担当 | typed `DurableStateBehavior` を同じ typed crate に追加するか、別 change に分けるか |
| journal / serializer の境界 | `Journal` は `AtomicWrite` を受け、persistence serializers は actor-core serialization registry に contributor として登録される。std local snapshot store は `SnapshotPayload` を serialization registry 経由で保存する | typed behavior DSL を追加する場合、serialization / adapter contract を effector と共有するか |
| durable state revision model | store trait は expected revision と optional tag を受け、tagged update metadata を返す | typed DurableStateBehavior がこの contract をどう実行するか |
| plugin adapter 境界 | core extension は generic journal / snapshot を直接受ける。proxy actor / installer と plugin message handler が追加済み | proxy extension を std adapter と組み合わせる integration pattern を docs / showcase で示すか |
| typed effector と Pekko typed DSL の境界 | `PersistenceEffector` は通常 `Behavior` に統合されるが、Pekko の `EffectBuilder` / signal / adapter をそのまま露出しない | parity 目標を effector-first で固定するか、Pekko direct DSL を追加するか |

## 実装優先度

ここで出す優先度は「今の要求で実装すべきか」ではなく、「Pekko parity ギャップをどの順で埋めるか」を示す。YAGNI は適用しない。以下は直前のカテゴリ別ギャップに列挙済みの項目だけを再配置する。

### Phase 1: trivial / easy

現時点で未実装の trivial / easy gap はない。

### Phase 2: medium

現時点で未実装の medium gap はない。前回 Phase 2 に置いていた `LocalSnapshotStore`、advanced plugin message hook、plugin proxy extension semantics、`EventSourcedSignal` family、`PublishedEvent` / `EventRejectedError`、behavior-level `onPersistFailure` は実装済みに移動した。

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

persistence は classic write-side と std snapshot adapter の基礎部品がかなり揃っている。`PersistentActor`、journal、snapshot、event adapter、serializer、at-least-once delivery、durable state store registry、in-memory store、local snapshot store、plugin proxy extension は存在し、panic 系スタブも見つからない。今回の再検証で adapter manifest と serializer manifest の接続、std local snapshot store、plugin proxy runtime semantics、typed event-sourced signal / published event / rejection error は実装済みに更新した。

parity を低コストで前進させる未実装機能は残っていない。次に残る差分は medium ではなく hard の typed behavior DSL / effect model に集中している。

parity 上の主要ギャップは、typed `EventSourcedBehavior` / `EffectBuilder` と typed `DurableStateBehavior` / durable state `EffectBuilder` である。typed write-side は effector-first API として前進しているが、Pekko parity の観点では behavior-level DSL と durable state behavior execution がまだ閉じていない。API カバレッジは 80% を超えたため、以後の設計判断は「effector-first を正とするか、Pekko direct DSL を追加するか」を先に決めるのが妥当である。
