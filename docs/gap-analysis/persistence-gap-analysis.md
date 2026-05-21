# persistence モジュール ギャップ分析

更新日: 2026-05-21 JST (current main 再検証)

## 比較スコープ定義

この調査は Apache Pekko persistence 配下の raw API 数をそのまま移植対象にするものではない。fraktor-rs の `persistence` では write-side persistence runtime を対象にし、`persistence-query`、testkit / TCK、Java / Scala DSL convenience、JVM 固有の plugin loading は parity 分母から除外する。

現行 fraktor-rs の persistence は、スキル定義に残っている旧 `modules/persistence-core/src/core/` ではなく、`modules/persistence-core-kernel/src/` と `modules/persistence-core-typed/src/` に分割済みである。`persistence-adaptor-std` はまだ存在しないため、ファイル IO や std runtime adapter は未実装ギャップとして扱うが、adaptor crate が未作成であること自体は減点しない。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| classic persistent actor | `modules/persistence-core-kernel/src/persistent/persistent_actor.rs`, `modules/persistence-core-kernel/src/persistent/eventsourced.rs`, `modules/persistence-core-kernel/src/persistent/persistence_context.rs` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/PersistentActor.scala`, `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Eventsourced.scala` |
| recovery / journal / snapshot | `modules/persistence-core-kernel/src/journal/`, `modules/persistence-core-kernel/src/snapshot/`, `modules/persistence-core-kernel/src/persistent/recovery.rs` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/`, `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/snapshot/`, `JournalProtocol.scala`, `SnapshotProtocol.scala` |
| persistent representation / adapter | `modules/persistence-core-kernel/src/persistent/persistent_repr.rs`, `modules/persistence-core-kernel/src/persistent/persistent_envelope.rs`, `modules/persistence-core-kernel/src/journal/event_adapters.rs`, `modules/persistence-core-kernel/src/journal/read_event_adapter.rs`, `modules/persistence-core-kernel/src/journal/write_event_adapter.rs`, `modules/persistence-core-kernel/src/journal/tagged.rs` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Persistent.scala`, `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/EventAdapter.scala`, `Tagged.scala` |
| durable state store contract | `modules/persistence-core-kernel/src/state/` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/state/scaladsl/`, `state/DurableStateStoreRegistry.scala`, `state/exception/DurableStateException.scala` |
| delivery / FSM compatibility | `modules/persistence-core-kernel/src/delivery/`, `modules/persistence-core-kernel/src/fsm/persistent_fsm.rs` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/AtLeastOnceDelivery.scala`, `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/fsm/PersistentFSM.scala` |
| plugin / extension / in-memory stores | `modules/persistence-core-kernel/src/extension/`, `modules/persistence-core-kernel/src/journal/persistence_plugin_proxy.rs`, `modules/persistence-core-kernel/src/journal/in_memory_journal.rs`, `modules/persistence-core-kernel/src/snapshot/in_memory_snapshot_store.rs` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Persistence.scala`, `journal/PersistencePluginProxy.scala`, `journal/inmem/InmemJournal.scala` |
| typed write-side API | `modules/persistence-core-typed/src/` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/`, `references/pekko-persistence-effector/library/src/main/scala/` |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| `persistence-query` | 固定スコープでは write-side runtime と別スコープ。ユーザーが query 調査を明示した場合だけ対象 |
| `persistence-testkit`, `persistence-tck`, `persistence-typed-tests` | runtime API ではない |
| `persistence-shared` の `src/test` 配下 | 現在の参照ツリーでは main runtime API がなく、shared LevelDB / serializer spec は test scope |
| JDBC / Cassandra / LevelDB など特定 storage plugin 完全互換 | storage backend 実装技術ごとの互換は別スコープ |
| Java DSL wrapper / `javadsl` package | Rust API として再現不要 |
| Scala implicit / package object / syntax sugar | Rust API として再現不要 |
| HOCON plugin loading / JVM reflection / classloader | JVM 固有 |
| replicated event sourcing / CRDT / typed reliable delivery queue | `persistence-typed` 内にあるが、現 persistence 固定スコープの列挙対象外。必要なら replication / delivery として別調査 |

### raw 抽出値の扱い

Pekko 側の固定スコープ候補ディレクトリを raw 抽出すると、型宣言 352 件、主要 `def` 1405 件が見つかる。これには private / internal / Java DSL / JVM 固有 / scope 外の replication 系 API が含まれるため、parity カバレッジ分母には使わない。

fraktor-rs 側は `modules/persistence-core-kernel/src/` と `modules/persistence-core-typed/src/` の `*_test.rs` / `lib_test.rs` を除外して raw 抽出した。raw public type declarations は 79 件（kernel: 59、typed: 20）、raw public method declarations は 277 件（kernel: 202、typed: 75）である。このうち外部到達可能な `pub` type declarations は 65 件（kernel: 55、typed: 10）で、`pub(crate)` の内部型は raw 参考値にのみ含める。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 80 |
| fraktor-rs 固定スコープ対応概念 | 58 |
| 固定スコープ概念カバレッジ | 58/80 (73%) |
| raw public type declarations | 79（kernel: 59, typed: 20） |
| raw public method declarations | 277（kernel: 202, typed: 75） |
| hard / medium / easy / trivial gap | 4 / 10 / 3 / 6 |
| panic 系スタブ | 0 件 |
| 機能 placeholder / TODO | 0 件 |

raw declaration count は参考値であり、parity 分母に使わない。

classic write-side persistence は、persistent actor、journal、snapshot、event adapter、at-least-once delivery、durable state store の基本契約が揃っている。typed write-side は Pekko typed `EventSourcedBehavior` / `Effect` の直移植ではなく、`pekko-persistence-effector` と同じく通常の typed `Behavior` を保ったまま hidden child store actor に永続化を委譲する effector-first API として実装されている。

2026-05-21 時点の再検証では、現行 crate 境界が引き続き `persistence-core-kernel` / `persistence-core-typed` であり、`persistence-adaptor-std` は存在しないことを確認した。typed effector は backoff 中の store command stashing、wait-state recovery signal handling、typed recovery selection、typed adapter contract、durable state signal surface まで進んでいるが、Pekko の typed behavior-level `onPersistFailure`、direct `EventSourcedBehavior` DSL、typed `DurableStateBehavior` はまだ parity ギャップとして残る。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / classic write-side | `PersistentActor`, `Eventsourced`, `Recovery`, journal, snapshot, adapter, delivery, durable state store | `Eventsourced`, `PersistentActor`, `Journal`, `SnapshotStore`, `EventAdapters`, `AtLeastOnceDelivery`, `DurableStateStore` が存在 | 主要契約は中程度以上に対応。AtomicWrite、revision、serializer、設定型が不足 |
| core / typed write-side | `EventSourcedBehavior`, `Effect`, signal, typed recovery / retention, `DurableStateBehavior` | `fraktor-persistence-core-typed-rs` が `PersistenceEffector`, `PersistenceEffectorConfig`, `PersistenceEffectorSignal`, `PersistenceMode`, `SnapshotCriteria`, `RetentionCriteria`, `BackoffConfig`, `PersistenceEffectorMessageAdapter`, `Recovery`, `SnapshotSelectionCriteria`, `EventAdapter`, `EventSeq`, `SnapshotAdapter`, `DurableStateSignal` を提供 | effector-first と Phase 1 typed parity surface は実装済み。Pekko 互換の behavior DSL と typed durable state behavior は未実装 |
| std / adaptor | local snapshot store、runtime plugin adapter | 対応 crate なし。in-memory store は kernel に存在 | ファイル IO / runtime adapter は未対応 |

## カテゴリ別ギャップ

ギャップ（未対応・部分実装・方針差あり）のみテーブルに列挙する。実装済みはカテゴリの件数カウントに含めるが、テーブル行には追加しない。

### 1. Persistent actor / recovery / lifecycle　✅ 実装済み 11/15 (73%)

fraktor-rs は Pekko の `PersistentActor` と複数 mix-in trait を、`Eventsourced` と `PersistentActor` に統合している。`persist` / `persist_async` / `persist_all` / `defer` / snapshot / delete / recovery callbacks は存在する。根拠は `modules/persistence-core-kernel/src/persistent/persistent_actor.rs:23`、`modules/persistence-core-kernel/src/persistent/eventsourced.rs:21`、`modules/persistence-core-kernel/src/persistent/eventsourced.rs:48`。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `RecoveryCompleted` signal type | `PersistentActor.scala:29`, `PersistentActor.scala:35` | 実装済み / non-goal | core | n/a | classic は `on_recovery_completed` callback と internal `JournalResponseAction::RecoveryCompleted` で表現済み。typed は `PersistenceEffectorSignal::RecoveryCompleted` を公開済み。Pekko 同名の classic 公開型追加は現設計では不要 |
| `SnapshotOffer` | `SnapshotProtocol.scala:157` | 実装済み | core | done | `SnapshotOffer` と `receive_snapshot_offer` を追加し、既存 `receive_snapshot` callback と互換 |
| `PersistenceSettings` | `Persistence.scala:40` | 実装済み | core/config | done | `PersistenceSettings` が `JournalActorConfig` / `SnapshotActorConfig` を束ね、store actor retry 設定を公開 |
| `AtomicWrite` | `Persistent.scala:45`, `Persistent.scala:49` | 部分実装 | core/journal | medium | `JournalMessage::WriteMessages` は `Vec<PersistentRepr>` を送るが、原子書き込み単位を表す公開型がない |

### 2. Journal / snapshot store protocol　✅ 実装済み 13/16 (81%)

`Journal` は Pekko の `AsyncWriteJournal` と `AsyncRecovery` を統合した trait として存在し、`JournalActor` / `JournalMessage` / `JournalResponse` もある。`SnapshotStore`、`SnapshotActor`、`SnapshotMessage`、`SnapshotResponse`、`SnapshotMetadata`、`SnapshotSelectionCriteria` も実装済み。根拠は `modules/persistence-core-kernel/src/journal/base.rs:9`、`modules/persistence-core-kernel/src/journal/journal_message.rs:15`、`modules/persistence-core-kernel/src/snapshot/snapshot_store.rs:10`、`modules/persistence-core-kernel/src/snapshot/snapshot_message.rs:17`。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `NoSnapshotStore` | `snapshot/NoSnapshotStore.scala:28` | 実装済み | core/snapshot | done | 何もしない `SnapshotStore` 実装 |
| `LocalSnapshotStore` | `snapshot/local/LocalSnapshotStore.scala:40` | 未対応 | std/snapshot | medium | ファイルシステム依存。core ではなく std adapter が妥当 |
| snapshot retry settings の公開契約 | `Persistence.scala:40`, `SnapshotProtocol.scala:235` | 実装済み | core/config | done | `SnapshotActorConfig` を `PersistenceSettings` から渡せる。plugin settings は fraktor-rs の trait 実装注入モデルでは non-goal |

### 3. Persistent representation / adapters / serialization　✅ 実装済み 11/14 (79%)

`PersistentRepr` は persistence id、sequence number、manifest、writer uuid、timestamp、deleted、sender、metadata を保持する。`WriteEventAdapter`、`ReadEventAdapter`、`IdentityEventAdapter`、`EventSeq`、`EventAdapters`、`Tagged` も存在する。根拠は `modules/persistence-core-kernel/src/persistent/persistent_repr.rs:20`、`modules/persistence-core-kernel/src/journal/write_event_adapter.rs:13`、`modules/persistence-core-kernel/src/journal/read_event_adapter.rs:14`、`modules/persistence-core-kernel/src/journal/event_adapters.rs:20`、`modules/persistence-core-kernel/src/journal/tagged.rs:16`。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| `MessageSerializer` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/serialization/MessageSerializer.scala:43` | 未対応 | kernel/serialization | medium | `PersistentRepr` / `AtomicWrite` / journal protocol の serialization contract がない |
| `SnapshotSerializer` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/serialization/SnapshotSerializer.scala:56` | 未対応 | kernel/serialization | medium | snapshot payload と metadata の serialization contract がない |
| adapter manifest と serializer manifest の接続 | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/EventAdapter.scala:42` | 部分実装 | kernel/serialization | medium | `WriteEventAdapter::manifest` 相当は `modules/persistence-core-kernel/src/journal/write_event_adapter.rs:17` にあるが、永続化 serializer registry との接続点がない |

### 4. At-least-once delivery / unconfirmed delivery　✅ 実装済み 6/7 (86%)

`AtLeastOnceDelivery`、`AtLeastOnceDeliveryConfig`、`AtLeastOnceDeliverySnapshot`、`UnconfirmedDelivery`、`UnconfirmedWarning`、`RedeliveryTick` は存在する。未確認配送の snapshot / restore、redelivery、confirm も実装済み。根拠は `modules/persistence-core-kernel/src/delivery/at_least_once_delivery.rs:21`、`modules/persistence-core-kernel/src/delivery/at_least_once_delivery.rs:72`、`modules/persistence-core-kernel/src/delivery/at_least_once_delivery.rs:103`。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `MaxUnconfirmedMessagesExceededException` 相当 | `AtLeastOnceDelivery.scala:80`, `AtLeastOnceDelivery.scala:126` | 実装済み | core/delivery | done | `PersistenceError::MaxUnconfirmedMessagesExceeded` を返す |

### 5. Durable State store contract　✅ 実装済み 5/8 (63%)

`DurableStateStore`、`DurableStateUpdateStore`、`DurableStateStoreProvider`、`DurableStateStoreRegistry`、`DurableStateError` は存在する。ただし Pekko の revision / tag を含む durable state write-side contract とはまだ差がある。根拠は `modules/persistence-core-kernel/src/state/durable_state_store.rs:12`、`modules/persistence-core-kernel/src/state/durable_state_update_store.rs:6`、`modules/persistence-core-kernel/src/state/durable_state_store_registry.rs:18`。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `GetObjectResult[A]` | `state/scaladsl/DurableStateStore.scala:31`, `state/scaladsl/DurableStateStore.scala:35` | 実装済み | core/durable_state | done | `GetObjectResult<A>` が value と revision を保持し、`DurableStateStore::get_object` が返す |
| revision / tag aware update store | `state/scaladsl/DurableStateUpdateStore.scala:37`, `state/scaladsl/DurableStateUpdateStore.scala:63` | 部分実装 | core/durable_state | medium | `upsert_object` と `delete_object` に revision / tag がない |
| `DeleteRevisionException` | `state/exception/DurableStateException.scala:41` | 実装済み | core/durable_state | done | `DurableStateError::DeleteRevision` を追加 |

### 6. Plugin / extension / in-memory stores　✅ 実装済み 5/7 (71%)

`PersistenceExtension`、`PersistenceExtensionId`、`PersistenceExtensionInstaller`、`PersistencePluginProxy`、`InMemoryJournal`、`InMemorySnapshotStore` は存在する。HOCON loading と runtime plugin id selection は対象外であり、Rust 側は `Journal` / `SnapshotStore` trait 実装を注入するモデルを採用する。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `RuntimePluginConfig` | `Persistence.scala:127` | non-goal | core/config | n/a | Pekko には存在するが、fraktor-rs は plugin 形式ではなく `Journal` / `SnapshotStore` trait 実装注入に留めるため不要 |
| plugin target location / proxy extension semantics | `journal/PersistencePluginProxy.scala:38`, `journal/PersistencePluginProxy.scala:85` | 部分実装 | core/plugin + std/runtime | medium | `PersistencePluginProxy<J, S>` は forwarding object だが、Pekko の target location / extension actor semantics まではない |

### 7. Persistent FSM compatibility　✅ 実装済み 1/1 (100%)

Pekko の `PersistentFSM` family は deprecated だが、固定スコープでは compatibility marker として確認した。fraktor-rs には最小契約の `PersistentFsm` trait が存在し、state transition event の persist / apply を `PersistentActor` 上で表現できる。根拠は `modules/persistence-core-kernel/src/fsm/persistent_fsm.rs:17`。

完全な FSM DSL、`FSMState`、`StateChangeEvent`、migration helper は Pekko 側でも legacy API であり、今回の parity 分母には含めない。

### 8. Typed write-side effector / EventSourcedBehavior / signal　✅ 実装済みまたは代替 5/9 (56%)

Pekko persistence の現行推奨 write-side API は typed `EventSourcedBehavior` と `Effect` 体系である。一方、fraktor-rs では本カテゴリを `EventSourcedBehavior` の 1:1 直移植ではなく、`pekko-persistence-effector` 由来の effector-first API として実装している。ユーザー actor は通常の typed `Behavior` のまま、内部 store actor が `PersistentActor` / `PersistenceContext` を使って recovery / persist / snapshot / retention を実行する。

実装済みまたは明確な代替とみなせるのは、`PersistenceId`、effector 経由の persist/snapshot 操作、`RetentionCriteria`、typed recovery selection、typed event/snapshot adapter contract である。Pekko 互換 DSL としての `EventSourcedBehavior` / `EffectBuilder`、published event、behavior-level persist failure supervision は未達である。根拠は `modules/persistence-core-typed/src/persistence_id.rs:7`、`modules/persistence-core-typed/src/persistence_effector.rs:31`、`modules/persistence-core-typed/src/persistence_effector.rs:205`、`modules/persistence-core-typed/src/retention_criteria.rs:5`、`modules/persistence-core-typed/src/recovery.rs`、`modules/persistence-core-typed/src/event_adapter.rs`、`modules/persistence-core-typed/src/snapshot_adapter.rs`。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| `EventSourcedBehavior[C,E,S]` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/EventSourcedBehavior.scala:36`, `EventSourcedBehavior.scala:138` | 方針差あり | typed | hard | 専用 DSL は導入せず、`modules/persistence-core-typed/src/persistence_effector.rs:53` の `PersistenceEffector::props(config, on_ready)` で通常 `Behavior` と統合 |
| `Effect` / `EffectBuilder` / `ReplyEffect` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/Effect.scala:132`, `Effect.scala:144`, `Effect.scala:196` | effector API で一部代替 | typed | hard | `modules/persistence-core-typed/src/persistence_effector.rs:205`、`persistence_effector.rs:233`、`persistence_effector.rs:258`、`persistence_effector.rs:287`、`persistence_effector.rs:327` で persist / snapshot callback を表現するが、reply/stash/unhandled/stop の effect model はない |
| `EventSourcedSignal` family | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/EventSourcedSignal.scala:27`, `EventSourcedSignal.scala:30`, `EventSourcedSignal.scala:139` | 部分実装 | typed | medium | `modules/persistence-core-typed/src/persistence_effector_signal.rs:11` が recovery / persisted / snapshot / delete / failure を actor-private message に包むが、Pekko の公開 signal family と一致しない |
| typed `Recovery` / typed `SnapshotSelectionCriteria` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/Recovery.scala:24`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/SnapshotSelectionCriteria.scala:21` | 実装済み | typed | done | `modules/persistence-core-typed/src/recovery.rs` と `modules/persistence-core-typed/src/snapshot_selection_criteria.rs` が crate root から re-export され、`PersistenceStoreActor::recovery()` で kernel `persistent::Recovery` へ変換される |
| typed `EventAdapter` / `EventSeq` / `SnapshotAdapter` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/EventAdapter.scala:35`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/SnapshotAdapter.scala:23` | 実装済み / snapshot runtime integration は non-goal | typed | done | `modules/persistence-core-typed/src/event_adapter.rs`、`event_seq.rs`、`snapshot_adapter.rs` を追加。event adapter は `PersistenceEffectorConfig::with_event_adapter` から kernel adapter registry へ接続し、snapshot adapter は public conversion contract に留める |
| `PublishedEvent` / `EventRejectedException` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/PublishedEvent.scala:28`, `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/EventRejectedException.scala:19` | 未対応 | typed | medium | event publication と rejection signal / error の公開契約がない |
| behavior-level `onPersistFailure` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/scaladsl/EventSourcedBehavior.scala:230` | 部分実装 | typed | medium | `modules/persistence-core-typed/src/backoff_config.rs:7`、`PersistenceEffectorConfig::with_backoff_config`、hidden store actor の `BackoffSupervisor` wiring はあるが、Pekko の behavior-level supervision hook としては未統合 |

### 9. Typed DurableStateBehavior　✅ 実装済み 1/3 (33%)

Durable state store contract は kernel に存在するが、Pekko typed の write-side behavior API は未実装である。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| `DurableStateBehavior[C,S]` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/DurableStateBehavior.scala:36`, `DurableStateBehavior.scala:127` | 未対応 | typed | hard | typed `Behavior` と durable state store の統合が必要。`withTag` / `onPersistFailure` も含む |
| durable state `Effect` / `EffectBuilder` / `ReplyEffect` | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/scaladsl/Effect.scala:124`, `Effect.scala:136`, `Effect.scala:188` | 未対応 | typed | hard | persist / delete / none / unhandled / stop / stash / reply の effect model が必要 |
| `DurableStateSignal` family | `references/pekko/persistence-typed/src/main/scala/org/apache/pekko/persistence/typed/state/DurableStateSignal.scala:26`, `DurableStateSignal.scala:29`, `DurableStateSignal.scala:33` | 実装済み | typed | done | `modules/persistence-core-typed/src/durable_state_signal.rs` が recovery completed / failed、state persisted / deleted、persistence failed を公開し、behavior implementation は non-goal |

## 対象外 (n/a / 固定スコープ外)

| Pekko API / 領域 | 判定理由 |
|------------------|----------|
| `persistence-query` | write-side runtime とは別スコープ |
| Java DSL wrapper / `javadsl/*` | Rust API として再現不要 |
| Scala syntax sugar / implicit ops | Rust API として再現不要 |
| HOCON dynamic loading / JVM reflection / classloader | JVM 固有 |
| `persistence-testkit`, `persistence-tck`, typed tests | runtime API ではない |
| JDBC / Cassandra / LevelDB plugin 完全互換 | storage backend 実装技術ごとの互換は別スコープ |
| full `PersistentFSM` DSL / migration helper | Pekko 側で legacy / deprecated。fraktor-rs は最小 `PersistentFsm` 契約を持つ |
| replicated event sourcing / CRDT / typed reliable delivery queue | 現 persistence 固定スコープ外。必要なら別スコープとして調査 |

## スタブ / placeholder

`modules/persistence-core-kernel/src` と `modules/persistence-core-typed/src` に対して `todo!()`、`unimplemented!()`、`panic!("not implemented")`、`TODO`、`FIXME`、`placeholder`、`stub` を検索した範囲では、公開 API 直下の未完成スタブは見つからなかった。

`PersistentActor::defer` / `defer_async` は recovery 中に panic するが、これは Pekko 互換の不正利用検出であり、未実装スタブではない。

## 実装優先度

### Phase 1: trivial / easy

| 項目 | 実装先層 | 根拠 | 状態 |
|------|----------|------|------|
| `SnapshotOffer` | core | カテゴリ1 | 実装済み |
| `PersistenceSettings` | core/config | カテゴリ1 | 実装済み |
| `NoSnapshotStore` | core/snapshot | カテゴリ2 | 実装済み |
| snapshot retry settings の公開契約 | core/config | カテゴリ2 | 実装済み |
| `MaxUnconfirmedMessagesExceededException` 相当 | core/delivery | カテゴリ4 | 実装済み |
| `GetObjectResult[A]` | core/durable_state | カテゴリ5 | 実装済み |
| `DeleteRevisionException` | core/durable_state | カテゴリ5 | 実装済み |
| typed `Recovery` / typed `SnapshotSelectionCriteria` | core/typed | カテゴリ8 | 実装済み |
| typed `EventAdapter` / `EventSeq` / `SnapshotAdapter` | core/typed | カテゴリ8 | 実装済み |
| `DurableStateSignal` family | core/typed | カテゴリ9 | 実装済み |

### Phase 2: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `AtomicWrite` | core/journal | カテゴリ1 |
| `LocalSnapshotStore` | std/snapshot | カテゴリ2 |
| `MessageSerializer` | core/serialization | カテゴリ3 |
| `SnapshotSerializer` | core/serialization | カテゴリ3 |
| adapter manifest と serializer manifest の接続 | core/serialization | カテゴリ3 |
| revision / tag aware update store | core/durable_state | カテゴリ5 |
| plugin target location / proxy extension semantics | core/plugin + std/runtime | カテゴリ6 |
| `EventSourcedSignal` family | core/typed | カテゴリ8 |
| `PublishedEvent` / `EventRejectedException` | core/typed | カテゴリ8 |

### Phase 3: hard

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `EventSourcedBehavior[C,E,S]` | core/typed | カテゴリ8 |
| `Effect` / `EffectBuilder` / `ReplyEffect` | core/typed | カテゴリ8 |
| `DurableStateBehavior[C,S]` | core/typed | カテゴリ9 |
| durable state `Effect` / `EffectBuilder` / `ReplyEffect` | core/typed | カテゴリ9 |

## 内部モジュール構造ギャップ

今回は API ギャップが支配的なため、内部モジュール構造ギャップの詳細分析は省略する。固定スコープ概念カバレッジは 58/80 (73%) で、特に typed `EventSourcedBehavior` direct DSL、typed `DurableStateBehavior`、serialization contract、durable state revision model が未達である。責務分割の細部比較より先に、behavior DSL と storage / serialization contract の境界を決める段階である。

次版で構造分析へ進む場合の観点は以下になる。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| classic と typed の境界 | `persistence-core-kernel` が classic runtime、`persistence-core-typed` が effector-first typed API を担当 | typed `DurableStateBehavior` を同じ typed crate に追加するか別 change に分けるか |
| journal / serializer の境界 | `Journal` は `PersistentRepr` を受けるが serialization contract がない | serializer registry を persistence-core に置くか actor-core serialization と接続するか |
| durable state revision model | store trait は value 中心で revision / tag を持たない | revision を store API に入れるか typed DurableStateBehavior 側に閉じるか |
| plugin adapter 境界 | core extension は generic journal / snapshot を直接受ける | std runtime で plugin selection / local snapshot store をどう表すか |
| typed effector と Pekko typed DSL の境界 | `PersistenceEffector` は通常 `Behavior` に統合されるが、Pekko の `EffectBuilder` / signal / adapter をそのまま露出しない | parity 目標を effector-first で固定するか、Pekko direct DSL を追加するか |

## まとめ

persistence は classic write-side の基礎部品はかなり揃っている。`PersistentActor`、journal、snapshot、event adapter、at-least-once delivery、durable state store registry、in-memory store は存在し、panic 系スタブも見つからない。

Phase 1 の kernel 側低コスト項目は `SnapshotOffer`、`PersistenceSettings`、`NoSnapshotStore`、snapshot retry settings、`MaxUnconfirmedMessagesExceededException` 相当、`GetObjectResult[A]`、`DeleteRevisionException` まで実装済みである。`RecoveryCompleted` の classic 同名公開型と `RuntimePluginConfig` / plugin settings は現設計では non-goal とした。

主要ギャップは、`AtomicWrite`、serialization contract、revision / tag-aware durable state update、typed `EventSourcedBehavior` / `EffectBuilder`、typed `DurableStateBehavior` である。typed write-side は effector-first API として前進し、Phase 1 typed parity surface は閉じたが、Pekko parity の観点では behavior-level failure supervision と durable state behavior execution がまだ閉じていない。内部構造比較は、serializer / revision model と typed durable state behavior の実行契約を決めた後に進めるのが妥当である。
