# persistence モジュール ギャップ分析

更新日: 2026-05-13 (typed effector 追加後)

## 比較スコープ定義

この調査は、Apache Pekko persistence 配下の raw API 数をそのまま移植対象にするものではない。fraktor-rs の `persistence` では write-side persistence runtime を対象にし、`persistence-query`、testkit / TCK、Java / Scala DSL convenience、JVM 固有の plugin loading は parity 分母から除外する。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| classic persistent actor | `modules/persistence-core-kernel/src/eventsourced.rs`, `persistent_actor.rs`, `persistence_context.rs` | `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/` |
| recovery / journal / snapshot | `journal*.rs`, `snapshot*.rs`, `recovery.rs` | `journal/`, `snapshot/`, `SnapshotProtocol.scala`, `JournalProtocol.scala` |
| persistent representation / adapter | `persistent_repr.rs`, `persistent_envelope.rs`, `event_adapters.rs`, `read_event_adapter.rs`, `write_event_adapter.rs`, `tagged.rs` | `Persistent.scala`, `journal/EventAdapter.scala`, `journal/Tagged.scala` |
| durable state store contract | `durable_state_store*.rs`, `durable_state_exception.rs` | `state/scaladsl/*`, `state/DurableStateStoreRegistry.scala`, `state/exception/*` |
| delivery / FSM compatibility | `at_least_once_delivery*.rs`, `unconfirmed_delivery.rs`, `persistent_fsm.rs` | `AtLeastOnceDelivery.scala`, `persistence/fsm/PersistentFSM.scala` |
| plugin / extension / in-memory stores | `persistence_extension*.rs`, `persistence_plugin_proxy.rs`, `in_memory_journal.rs`, `in_memory_snapshot_store.rs` | `Persistence.scala`, `journal/PersistencePluginProxy.scala`, `journal/inmem/InmemJournal.scala` |
| typed write-side API | `modules/persistence-core-typed/src` の `PersistenceEffector` API | `references/pekko/persistence-typed/src/main/scala/` と `references/pekko-persistence-effector/` |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| `persistence-query` | 固定スコープでは write-side runtime と別スコープ。ユーザーが query 調査を明示した場合だけ対象 |
| `persistence-testkit`, `persistence-tck`, `persistence-typed-tests` | runtime API ではない |
| `persistence-shared` の `src/test` 配下 | 現在の参照ツリーでは main runtime API がなく、shared LevelDB / serializer spec は test scope |
| JDBC / Cassandra / LevelDB など特定 storage plugin 完全互換 | storage backend 実装技術ごとの互換は別スコープ |
| Java DSL wrapper / javadsl package | Rust API として再現不要 |
| Scala implicit / package object / syntax sugar | Rust API として再現不要 |
| HOCON plugin loading / JVM reflection / classloader | JVM 固有 |
| replicated event sourcing / CRDT / typed reliable delivery queue | `persistence-typed` 内にあるが、現 persistence 固定スコープの列挙対象外。必要なら replication / delivery として別調査 |

### raw 抽出値の扱い

固定スコープ候補ディレクトリを raw 抽出すると、Pekko 側は型宣言 352 件、主要 `def` 1405 件が見つかる。これには private / internal / Java DSL / JVM 固有 / scope 外の replication 系 API が含まれるため、parity カバレッジ分母には使わない。

fraktor-rs 側の classic runtime は `modules/persistence-core-kernel/src/` に分離済みで、typed write-side は `modules/persistence-core-typed/src/` の effector API として追加済みである。`persistence-adaptor-std` はまだ存在しない。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 約 100 |
| fraktor-rs 固定スコープ対応概念 | 約 51 |
| 固定スコープ概念カバレッジ | 約 51/100 (51%) |
| hard gap | 4 |
| medium gap | 10 |
| easy gap | 7 |
| trivial gap | 6 |
| panic 系スタブ | 0 件 |
| 機能 placeholder / TODO | 0 件 |

classic write-side persistence は、persistent actor、journal、snapshot、event adapter、at-least-once delivery、durable state store の基本契約が揃っている。typed write-side は Pekko typed `EventSourcedBehavior` / `Effect` の直移植ではなく、`pekko-persistence-effector` と同じく通常の typed `Behavior` を保ったまま hidden child store actor に永続化を委譲する effector-first API として実装した。

旧版は `persistence-query` を分母に含めていたため、write-side runtime の評価としてはスコープが混在していた。固定スコープ版では query を外し、代わりに typed write-side API を parity 対象として明示する。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / classic write-side | `PersistentActor`, `Recovery`, journal, snapshot, adapter, delivery, durable state store | `Eventsourced`, `PersistentActor`, `Journal`, `SnapshotStore`, `EventAdapters`, `AtLeastOnceDelivery`, `DurableStateStore` が存在 | 主要契約は中程度以上に対応。AtomicWrite、revision、serializer、設定型が不足 |
| core / typed write-side | `EventSourcedBehavior`, `Effect`, signal, typed recovery / retention, `DurableStateBehavior` | `fraktor-persistence-core-typed-rs` が `PersistenceEffector`, `PersistenceEffectorConfig`, `PersistenceEffectorSignal`, `PersistenceMode`, `SnapshotCriteria`, `RetentionCriteria` を提供 | effector-first は実装済み。`EventSourcedBehavior` / typed durable state は未実装 |
| std / adapter | local snapshot store、runtime plugin adapter | 対応 crate なし。in-memory store は core に存在 | ファイル IO / runtime adapter は未対応 |

## カテゴリ別ギャップ

### 1. Persistent actor / recovery / lifecycle　✅ 実装済み 11/15 (73%)

fraktor-rs は Pekko の `PersistentActor` と複数 mix-in trait を、`Eventsourced` と `PersistentActor` に統合している。`persist` / `persist_async` / `persist_all` / `defer` / snapshot / delete / recovery callbacks は存在する。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `RecoveryCompleted` signal type | `PersistentActor.scala:29`, `PersistentActor.scala:35` | 部分実装 | core | trivial | `on_recovery_completed` callback はあるが、明示的な signal 型はない |
| `SnapshotOffer` | `SnapshotProtocol.scala:157` | 部分実装 | core | trivial | `receive_snapshot` callback はあるが、recovery signal としての型はない |
| `PersistenceSettings` | `Persistence.scala:40` | 未対応 | core/config | easy | recovery timeout、stash、journal / snapshot 設定を束ねる公開設定型がない |
| `AtomicWrite` | `Persistent.scala:45`, `Persistent.scala:49` | 部分実装 | core/journal | medium | `JournalMessage::WriteMessages` は `Vec<PersistentRepr>` を送るが、原子書き込み単位を表す公開型がない |

### 2. Journal / snapshot store protocol　✅ 実装済み 13/16 (81%)

`Journal` は Pekko の `AsyncWriteJournal` と `AsyncRecovery` を統合した trait として存在し、`JournalActor` / `JournalMessage` / `JournalResponse` もある。`SnapshotStore`、`SnapshotActor`、`SnapshotMessage`、`SnapshotResponse`、`SnapshotMetadata`、`SnapshotSelectionCriteria` も実装済み。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `NoSnapshotStore` | `snapshot/NoSnapshotStore.scala:28` | 未対応 | core/snapshot | trivial | 何もしない `SnapshotStore` 実装 |
| `LocalSnapshotStore` | `snapshot/local/LocalSnapshotStore.scala:40` | 未対応 | std/snapshot | medium | ファイルシステム依存。core ではなく std adapter が妥当 |
| snapshot retry / plugin settings の公開契約 | `Persistence.scala:40`, `SnapshotProtocol.scala:235` | 部分実装 | core/config | easy | actor config はあるが、Pekko 互換の persistence settings として統合されていない |

### 3. Persistent representation / adapters / serialization　✅ 実装済み 11/14 (79%)

`PersistentRepr` は persistence id、sequence number、manifest、writer uuid、timestamp、deleted、sender、metadata を保持する。`WriteEventAdapter`、`ReadEventAdapter`、`IdentityEventAdapter`、`EventSeq`、`EventAdapters`、`Tagged` も存在する。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `MessageSerializer` | `serialization/MessageSerializer.scala:43` | 未対応 | core/serialization | medium | `PersistentRepr` / `AtomicWrite` / journal protocol の serialization contract がない |
| `SnapshotSerializer` | `serialization/SnapshotSerializer.scala:56` | 未対応 | core/serialization | medium | snapshot payload と metadata の serialization contract がない |
| adapter manifest と serializer manifest の接続 | `journal/EventAdapter.scala:42`, `serialization/MessageSerializer.scala:43` | 部分実装 | core/serialization | medium | `WriteEventAdapter::manifest` はあるが、永続化 serializer registry との接続点がない |

### 4. At-least-once delivery / unconfirmed delivery　✅ 実装済み 6/7 (86%)

`AtLeastOnceDelivery`、`AtLeastOnceDeliveryConfig`、`AtLeastOnceDeliverySnapshot`、`UnconfirmedDelivery`、`UnconfirmedWarning`、`RedeliveryTick` は存在する。未確認配送の snapshot / restore、redelivery、confirm も実装済み。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `MaxUnconfirmedMessagesExceededException` 相当 | `AtLeastOnceDelivery.scala:80`, `AtLeastOnceDelivery.scala:126` | 部分実装 | core/delivery | trivial | 現状は `PersistenceError::MessagePassing("max unconfirmed deliveries exceeded")`。専用 error variant がない |

### 5. Durable State store contract　✅ 実装済み 5/8 (63%)

`DurableStateStore`、`DurableStateUpdateStore`、`DurableStateStoreProvider`、`DurableStateStoreRegistry`、`DurableStateError` は存在する。ただし Pekko の revision / tag を含む durable state write-side contract とはまだ差がある。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `GetObjectResult[A]` | `state/scaladsl/DurableStateStore.scala:31`, `state/scaladsl/DurableStateStore.scala:35` | 未対応 | core/durable_state | trivial | 現状は `Option<A>` だけを返し、revision を保持しない |
| revision / tag aware update store | `state/scaladsl/DurableStateUpdateStore.scala:37`, `state/scaladsl/DurableStateUpdateStore.scala:63` | 部分実装 | core/durable_state | medium | `upsert_object` と `delete_object` に revision / tag がない |
| `DeleteRevisionException` | `state/exception/DurableStateException.scala:41` | 未対応 | core/durable_state | trivial | `DurableStateError` に revision mismatch 系の variant がない |

### 6. Plugin / extension / in-memory stores　✅ 実装済み 5/7 (71%)

`PersistenceExtension`、`PersistenceExtensionId`、`PersistenceExtensionInstaller`、`PersistencePluginProxy`、`InMemoryJournal`、`InMemorySnapshotStore` は存在する。HOCON loading は対象外だが、Rust 側にも runtime plugin selection の公開契約はまだ薄い。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `RuntimePluginConfig` | `Persistence.scala:127` | 未対応 | core/config | easy | HOCON ではなく Rust の型付き plugin config として定義可能 |
| plugin target location / proxy extension semantics | `journal/PersistencePluginProxy.scala:38`, `journal/PersistencePluginProxy.scala:85` | 部分実装 | core/plugin + std/runtime | medium | `PersistencePluginProxy<J, S>` は forwarding object だが、Pekko の target location / extension actor semantics まではない |

### 7. Persistent FSM compatibility　✅ 実装済み 1/1 (100%)

Pekko の `PersistentFSM` family は deprecated だが、固定スコープでは compatibility marker として確認した。fraktor-rs には最小契約の `PersistentFsm` trait が存在し、state transition event の persist / apply を `PersistentActor` 上で表現できる。

完全な FSM DSL、`FSMState`、`StateChangeEvent`、migration helper は Pekko 側でも legacy API であり、今回の parity 分母には含めない。

### 8. Typed write-side effector / EventSourcedBehavior / signal　✅ effector-first 実装済み

Pekko persistence の現行推奨 write-side API は typed `EventSourcedBehavior` と `Effect` 体系である。一方、fraktor-rs では本カテゴリを `EventSourcedBehavior` の 1:1 直移植ではなく、`pekko-persistence-effector` 由来の effector-first API として実装した。ユーザー actor は通常の typed `Behavior` のまま、内部 store actor が `PersistentActor` / `PersistenceContext` を使って recovery / persist / snapshot / retention を実行する。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `PersistenceId` | `PersistenceId.scala:16`, `PersistenceId.scala:155` | 実装済み | typed | easy | `of_unique_id` / `of_entity_id` / `as_str` |
| `EventSourcedBehavior[C,E,S]` | `scaladsl/EventSourcedBehavior.scala:36`, `scaladsl/EventSourcedBehavior.scala:138` | non-goal | typed | hard | 専用 DSL は導入せず、`PersistenceEffector::props(config, on_ready)` で通常 `Behavior` と統合 |
| `Effect` / `EffectBuilder` / `ReplyEffect` | `scaladsl/Effect.scala:132`, `scaladsl/Effect.scala:144`, `scaladsl/Effect.scala:196` | effector API で代替 | typed | hard | `persist_event`, `persist_events`, `persist_snapshot`, `persist_event(s)_with_snapshot` と `FnOnce` callback で表現 |
| `EventSourcedSignal` family | `EventSourcedSignal.scala:27`, `EventSourcedSignal.scala:30`, `EventSourcedSignal.scala:139` | 部分実装 | typed | medium | `PersistenceEffectorSignal` が recovery / persisted / snapshot / delete / failure を actor-private message に包む |
| typed `Recovery` / typed `SnapshotSelectionCriteria` | `scaladsl/Recovery.scala:24`, `SnapshotSelectionCriteria.scala:21` | 部分実装 | typed | easy | recovery は store actor に隠蔽。snapshot 判定は `SnapshotCriteria` として提供 |
| `RetentionCriteria` | `scaladsl/RetentionCriteria.scala:24`, `scaladsl/RetentionCriteria.scala:31` | 実装済み | typed | medium | snapshot 成功後に保持対象外 snapshot deletion を store actor へ送る |
| typed `EventAdapter` / `EventSeq` / `SnapshotAdapter` | `EventAdapter.scala:35`, `EventAdapter.scala:84`, `SnapshotAdapter.scala:23` | 未対応 | core/typed | easy | classic adapter はあるが、型パラメータ付き wrapper がない |
| `PublishedEvent` / `EventRejectedException` | `PublishedEvent.scala:28`, `EventRejectedException.scala:19` | 未対応 | core/typed | medium | event publication と rejection signal / error の公開契約がない |

### 9. Typed DurableStateBehavior　✅ 実装済み 0/8 (0%)

Durable state store contract は存在するが、Pekko typed の write-side behavior API は未実装である。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `DurableStateBehavior[C,S]` | `state/scaladsl/DurableStateBehavior.scala:36`, `state/scaladsl/DurableStateBehavior.scala:127` | 未対応 | core/typed | hard | typed `Behavior` と durable state store の統合が必要 |
| durable state `Effect` / `EffectBuilder` / `ReplyEffect` | `state/scaladsl/Effect.scala:124`, `state/scaladsl/Effect.scala:136`, `state/scaladsl/Effect.scala:188` | 未対応 | core/typed | hard | persist / delete / none / unhandled / stop / stash / reply の effect model が必要 |
| `DurableStateSignal` family | `state/DurableStateSignal.scala:26`, `state/DurableStateSignal.scala:33` | 未対応 | core/typed | easy | RecoveryCompleted / RecoveryFailed を typed signal として表す契約 |

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

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `RecoveryCompleted` signal type | core | カテゴリ1 |
| `SnapshotOffer` | core | カテゴリ1 |
| `PersistenceSettings` | core/config | カテゴリ1 |
| `NoSnapshotStore` | core/snapshot | カテゴリ2 |
| snapshot retry / plugin settings の公開契約 | core/config | カテゴリ2 |
| `MaxUnconfirmedMessagesExceededException` 相当 | core/delivery | カテゴリ4 |
| `GetObjectResult[A]` | core/durable_state | カテゴリ5 |
| `DeleteRevisionException` | core/durable_state | カテゴリ5 |
| `RuntimePluginConfig` | core/config | カテゴリ6 |
| typed `Recovery` / typed `SnapshotSelectionCriteria` | core/typed | カテゴリ8 |
| typed `EventAdapter` / `EventSeq` / `SnapshotAdapter` | core/typed | カテゴリ8 |
| `DurableStateSignal` family | core/typed | カテゴリ9 |

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

今回は API ギャップが支配的なため、内部モジュール構造ギャップの詳細分析は省略する。固定スコープ概念カバレッジは約 51% で、typed write-side API が未実装のため、責務分割の細部比較より先に公開契約と typed layer の有無を閉じる段階である。

次版で構造分析へ進む場合の観点は以下になる。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| classic と typed の境界 | `persistence-core-kernel` が classic runtime、`persistence-core-typed` が effector-first typed API を担当 | typed `DurableStateBehavior` を同じ typed crate に追加するか別 change に分けるか |
| journal / serializer の境界 | `Journal` は `PersistentRepr` を受けるが serialization contract がない | serializer registry を persistence-core に置くか actor-core serialization と接続するか |
| durable state revision model | store trait は value 中心で revision / tag を持たない | revision を store API に入れるか typed DurableStateBehavior 側に閉じるか |
| plugin adapter 境界 | core extension は generic journal / snapshot を直接受ける | std runtime で plugin selection / local snapshot store をどう表すか |

## まとめ

persistence は classic write-side の基礎部品はかなり揃っている。`PersistentActor`、journal、snapshot、event adapter、at-least-once delivery、durable state store registry、in-memory store は存在し、panic 系スタブも見つからない。

低コストで parity を前進できるのは、`RecoveryCompleted` / `SnapshotOffer` の signal 型、`NoSnapshotStore`、durable state の `GetObjectResult` / `DeleteRevisionException`、`PersistenceId`、typed recovery / adapter wrapper である。

主要ギャップは、`AtomicWrite`、serialization contract、revision / tag aware durable state update、typed `DurableStateBehavior` である。typed `EventSourcedBehavior` と `Effect` 体系は 1:1 移植ではなく effector-first API で扱う方針に更新済みである。内部構造比較は、serializer / revision model と typed durable state の公開契約を決めた後に進めるのが妥当である。
