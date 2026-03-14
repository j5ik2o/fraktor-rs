# persistence モジュール ギャップ分析

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（Scala API） | 119（classic: 47, typed: 44, query: 28） |
| Pekko 非推奨型数 | 4（PersistentFSM ファミリー） |
| fraktor-rs 公開型数 | 51（すべて core 層） |
| カバレッジ（型単位、classic のみ） | 33/47 (70%) |
| カバレッジ（全体） | 33/119 (28%) |
| ギャップ数（classic） | 14 |
| ギャップ数（typed） | 44（全未実装） |
| ギャップ数（query） | 28（全未実装） |

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core（classic 永続化基盤） | 47 | 33 | 70% |
| core/typed（型付き永続化） | 44 | 0 | 0% |
| std（クエリ・ストリーム統合） | 28 | 0 | 0% |

**注**: fraktor-rs の persistence モジュールは現在 `core/` 層のみ（no_std）。`std/` 層は未作成。
`typed/` サブ層も存在しない。

## Pekko モジュールと fraktor-rs の対応

| Pekko モジュール | fraktor-rs 対応 | 状態 |
|------------------|----------------|------|
| `pekko-persistence`（classic） | `modules/persistence/src/core/` | 主要機能を実装済み (70%) |
| `pekko-persistence-typed` | 未対応 | 全未実装 |
| `pekko-persistence-query` | 未対応 | 全未実装 |
| `pekko-persistence-testkit` | InMemoryJournal / InMemorySnapshotStore のみ | 部分的 |
| `pekko-persistence-tck` | 未対応 | 全未実装 |

---

## カテゴリ別ギャップ

### 1. コア（永続化アクター基盤）　✅ 実装済み 10/13 (77%)

fraktor-rs では Pekko の `PersistentActor` + 複数ミキシントレイト（`PersistenceIdentity`, `PersistenceRecovery`, `PersistenceStash`, `Snapshotter`）を `Eventsourced` + `PersistentActor` の2つのトレイトに統合している。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RecoveryCompleted` | `PersistentActor.scala:L35` | 未対応 | core | trivial | `on_recovery_completed` コールバックは存在するが、明示的なシグナル型がない |
| `PersistenceSettings` | `Persistence.scala:L40` | 未対応 | core | easy | AtLeastOnceDeliveryConfig が部分的に対応 |
| `AtomicWrite` | `Persistent.scala:L49` | 未対応 | core | medium | バッチ書き込みの原子性単位。Journal trait の API 拡張が必要 |

### 2. ジャーナル　✅ 実装済み 5/5 (100%)

Pekko の `AsyncWriteJournal` + `AsyncRecovery` を fraktor-rs では単一の `Journal` trait に統合。
`JournalActor`, `JournalActorConfig`, `JournalMessage`, `JournalResponse`, `JournalError` は fraktor-rs 固有のインフラ型。

ギャップなし。

### 3. スナップショット　✅ 実装済み 6/9 (67%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SnapshotOffer` | `SnapshotProtocol.scala:L157` | 未対応 | core | trivial | リカバリ中にスナップショットを提示するシグナル。`receive_snapshot` コールバックで代替 |
| `NoSnapshotStore` | `snapshot/NoSnapshotStore.scala` | 未対応 | core | trivial | 何もしない SnapshotStore 実装 |
| `LocalSnapshotStore` | `snapshot/local/LocalSnapshotStore.scala` | 未対応 | std | medium | ファイルシステムベースのスナップショットストア。std/tokio 依存 |

### 4. イベントアダプタ　✅ 実装済み 6/6 (100%)

fraktor-rs は Pekko の `EventAdapter`（統合トレイト）を `WriteEventAdapter` + `ReadEventAdapter` に分離。
`EventAdapters`（レジストリ）、`IdentityEventAdapter`、`EventSeq` もすべて実装済み。

ギャップなし。

### 5. 少なくとも1回配送　✅ 実装済み 4/5 (80%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `MaxUnconfirmedMessagesExceededException` | `AtLeastOnceDelivery.scala:L80` | 未対応 | core | trivial | `PersistenceError` のバリアントとして追加可能 |

### 6. 永続化状態（Durable State）　✅ 実装済み 5/7 (71%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `GetObjectResult[A]` | `state/scaladsl/DurableStateStore.scala:L35` | 未対応 | core | trivial | `(Option<A>, revision: u64)` の構造体。戻り値型の明示化 |
| `DeleteRevisionException` | `state/exception/DurableStateException.scala:L41` | 未対応 | core | trivial | `DurableStateException` に `DeleteRevision` バリアント追加で対応可能 |

### 7. シリアライゼーション　✅ 実装済み 0/2 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `MessageSerializer` | `serialization/MessageSerializer.scala:L43` | 未対応 | core | medium | PersistentRepr / AtomicWrite 等のシリアライズ。serde 統合が必要 |
| `SnapshotSerializer` | `serialization/SnapshotSerializer.scala:L56` | 未対応 | core | medium | スナップショットデータのシリアライズ |

### 8. 設定・プラグイン　✅ 実装済み 0/2 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RuntimePluginConfig` | `Persistence.scala:L127` | 未対応 | core | easy | ジャーナル/スナップショットプラグインの動的設定 |
| `StashOverflowStrategyConfigurator` | `PersistentActor.scala:L160` | 未対応 | core | trivial | 設定からの StashOverflowStrategy 解決 |

### 9. FSM（非推奨）　n/a 1/4 (25%)

Pekko で `@deprecated("Use EventSourcedBehavior", "Akka 2.6.0")` とされている。
fraktor-rs には `PersistentFsm` トレイトが存在する（最小限のコントラクト）。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| FSM DSL (`when`/`goto`/`stay`/`stop`) | `PersistentFSMBase.scala` | 未対応 | n/a | n/a | Pekko で非推奨。EventSourcedBehavior への移行推奨 |
| `FSMState` | `PersistentFSM.scala:L238` | 未対応 | n/a | n/a | 同上 |
| `StateChangeEvent` | `PersistentFSM.scala:L218` | 未対応 | n/a | n/a | 同上 |

### 10. 型付き永続化（persistence-typed）　✅ 実装済み 0/44 (0%)

persistence-typed は Pekko の最新の永続化 API であり、classic API に代わる推奨アプローチ。
fraktor-rs では**全く未実装**。

#### 10a. EventSourcedBehavior ファミリー

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `EventSourcedBehavior[C,E,S]` | `scaladsl/EventSourcedBehavior.scala:L138` | 未対応 | core/typed | hard | typed 層の基盤。Behavior<M> との統合が必要 |
| `PersistenceId` | `PersistenceId.scala:L155` | 未対応 | core | easy | entityTypeHint + entityId の構造化 ID |
| `Effect[Event, State]` | `scaladsl/Effect.scala:L132` | 未対応 | core/typed | hard | persist/none/unhandled/stop/stash/reply 等のエフェクト体系 |
| `EffectBuilder[Event, State]` | `scaladsl/Effect.scala:L144` | 未対応 | core/typed | hard | thenRun/thenStop/thenReply 等のチェーンAPI |
| `ReplyEffect[Event, State]` | `scaladsl/Effect.scala:L196` | 未対応 | core/typed | hard | 返信を強制する Effect |

#### 10b. シグナル型

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `EventSourcedSignal` | `EventSourcedSignal.scala:L27` | 未対応 | core/typed | medium | sealed trait。コールバック型より型安全 |
| `RecoveryCompleted` | `EventSourcedSignal.scala:L30` | 部分実装 | core/typed | trivial | コールバックは存在 |
| `RecoveryFailed` | `EventSourcedSignal.scala:L34` | 部分実装 | core/typed | trivial | コールバックは存在 |
| `JournalPersistFailed` | `EventSourcedSignal.scala:L42` | 部分実装 | core/typed | trivial | コールバックは存在 |
| `JournalPersistRejected` | `EventSourcedSignal.scala:L50` | 部分実装 | core/typed | trivial | コールバックは存在 |
| `SnapshotCompleted` | `EventSourcedSignal.scala:L58` | 未対応 | core/typed | trivial | |
| `SnapshotFailed` | `EventSourcedSignal.scala:L66` | 部分実装 | core/typed | trivial | コールバックは存在 |
| `DeleteSnapshotsCompleted` | `EventSourcedSignal.scala:L110` | 未対応 | core/typed | trivial | |
| `DeleteSnapshotsFailed` | `EventSourcedSignal.scala:L118` | 未対応 | core/typed | trivial | |
| `DeleteEventsCompleted` | `EventSourcedSignal.scala:L131` | 未対応 | core/typed | trivial | |
| `DeleteEventsFailed` | `EventSourcedSignal.scala:L139` | 未対応 | core/typed | trivial | |
| `DeletionTarget` | `EventSourcedSignal.scala:L156` | 未対応 | core/typed | trivial | |
| `SnapshotMetadata`(typed) | `EventSourcedSignal.scala:L105` | 未対応 | core/typed | trivial | classic 版は実装済み |

#### 10c. リカバリ・リテンション

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Recovery`(typed) | `scaladsl/Recovery.scala:L24` | 未対応 | core/typed | easy | default/disabled/withSnapshotSelectionCriteria |
| `RetentionCriteria` | `scaladsl/RetentionCriteria.scala:L24` | 未対応 | core/typed | medium | スナップショット後のイベント・スナップショット削除ポリシー |
| `SnapshotCountRetentionCriteria` | `scaladsl/RetentionCriteria.scala:L54` | 未対応 | core/typed | medium | N イベントごとにスナップショット + 古いスナップショットの削除 |

#### 10d. DurableStateBehavior

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `DurableStateBehavior[C,S]` | `state/scaladsl/DurableStateBehavior.scala:L127` | 未対応 | core/typed | hard | CQRS 書き込み側。Effect 体系が必要 |
| `Effect[State]`(DS) | `state/scaladsl/Effect.scala:L124` | 未対応 | core/typed | hard | persist/delete/none/unhandled/stop/stash/reply |
| `EffectBuilder[State]`(DS) | `state/scaladsl/Effect.scala:L136` | 未対応 | core/typed | hard | |
| `ReplyEffect[State]`(DS) | `state/scaladsl/Effect.scala:L188` | 未対応 | core/typed | hard | |
| `DurableStateSignal` | `state/DurableStateSignal.scala:L26` | 未対応 | core/typed | easy | RecoveryCompleted / RecoveryFailed のみ |

#### 10e. アダプタ（typed 版）

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `EventAdapter[E,P]`(typed) | `EventAdapter.scala:L35` | 未対応 | core/typed | easy | 型パラメータ付き。classic 版は実装済み |
| `EventSeq[A]`(typed) | `EventAdapter.scala:L78` | 未対応 | core/typed | easy | 型パラメータ付き。classic 版は実装済み |
| `SnapshotAdapter[State]` | `SnapshotAdapter.scala:L23` | 未対応 | core/typed | easy | スナップショットのシリアライズ変換 |

#### 10f. その他（typed）

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `PublishedEvent` | `PublishedEvent.scala:L28` | 未対応 | core/typed | medium | イベント発行（pub/sub） |
| `EventRejectedException` | `EventRejectedException.scala:L19` | 未対応 | core/typed | trivial | |
| `SnapshotSelectionCriteria`(typed) | `SnapshotSelectionCriteria.scala:L50` | 未対応 | core/typed | easy | classic 版は実装済み。typed ラッパー |

### 11. レプリケーション / CRDT　✅ 実装済み 0/8 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ReplicationContext` | `scaladsl/ReplicatedEventSourcing.scala:L29` | 未対応 | core/typed | hard | マルチデータセンター対応の文脈情報 |
| `ReplicatedEventSourcing` | `scaladsl/ReplicatedEventSourcing.scala:L78` | 未対応 | core/typed | hard | レプリケーション設定のファクトリ |
| `ReplicationId` | `ReplicationId.scala:L42` | 未対応 | core | easy | typeName + entityId + replicaId の構造化 ID |
| `ReplicaId` | `ReplicaId.scala:L19` | 未対応 | core | trivial | レプリカ識別子（newtype） |
| `OpCrdt[Operation]` | `crdt/OpCrdt.scala:L19` | 未対応 | core | medium | Operation-based CRDT の基底トレイト |
| `Counter` | `crdt/Counter.scala:L33` | 未対応 | core | easy | G-Counter 実装 |
| `LwwTime` | `crdt/LwwTime.scala:L21` | 未対応 | core | easy | Last-Writer-Wins タイムスタンプ |
| `ORSet[A]` | `crdt/ORSet.scala:L287` | 未対応 | core | hard | Observed-Remove Set。複雑な delta-CRDT |

### 12. 信頼性のあるデリバリ（typed）　✅ 実装済み 0/2 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `EventSourcedProducerQueue` | `delivery/EventSourcedProducerQueue.scala:L46` | 未対応 | core/typed | hard | 永続化ベースのプロデューサーキュー |
| `Settings` | `delivery/EventSourcedProducerQueue.scala:L88` | 未対応 | core/typed | easy | ProducerQueue の設定 |

### 13. クエリ（persistence-query）　✅ 実装済み 0/28 (0%)

persistence-query モジュールは CQRS の読み取り側を担当する。fraktor-rs では全く未実装。

#### 13a. オフセット型

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Offset` | `query/Offset.scala:L33` | 未対応 | core | easy | 読み取りオフセットの基底型 |
| `Sequence` | `query/Offset.scala:L44` | 未対応 | core | trivial | シーケンス番号ベースのオフセット |
| `TimeBasedUUID` | `query/Offset.scala:L57` | 未対応 | core | trivial | UUID ベースのオフセット |
| `TimestampOffset` | `query/Offset.scala:L105` | 未対応 | core | easy | タイムスタンプベースのオフセット |
| `NoOffset` | `query/Offset.scala:L125` | 未対応 | core | trivial | 最初から読む |

#### 13b. イベントエンベロープ

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `EventEnvelope` | `query/EventEnvelope.scala:L50` | 未対応 | core | easy | offset + persistenceId + seqNr + event |
| `EventEnvelope[Event]`(typed) | `query/typed/EventEnvelope.scala:L63` | 未対応 | core/typed | easy | 型付き版 |

#### 13c. Durable State 変更型

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `DurableStateChange[A]` | `query/DurableStateChange.scala:L28` | 未対応 | core | easy | 変更ストリームの基底型 |
| `UpdatedDurableState[A]` | `query/DurableStateChange.scala:L56` | 未対応 | core | trivial | |
| `DeletedDurableState[A]` | `query/DurableStateChange.scala:L78` | 未対応 | core | trivial | |

#### 13d. プラグイン SPI

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ReadJournalProvider` | `query/ReadJournalProvider.scala:L28` | 未対応 | core | easy | プラグインファクトリ |
| `PersistenceQuery` Extension | `query/PersistenceQuery.scala:L48` | 未対応 | core | medium | ActorSystem からクエリ取得 |

#### 13e. クエリトレイト

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ReadJournal` | `query/scaladsl/ReadJournal.scala:L37` | 未対応 | core | trivial | マーカートレイト |
| `EventsByPersistenceIdQuery` | `query/scaladsl/EventsByPersistenceIdQuery.scala` | 未対応 | std | medium | Source (ストリーム) 依存 |
| `CurrentEventsByPersistenceIdQuery` | `query/scaladsl/CurrentEventsByPersistenceIdQuery.scala` | 未対応 | std | medium | 同上 |
| `EventsByTagQuery` | `query/scaladsl/EventsByTagQuery.scala` | 未対応 | std | medium | 同上 |
| `CurrentEventsByTagQuery` | `query/scaladsl/CurrentEventsByTagQuery.scala` | 未対応 | std | medium | 同上 |
| `PersistenceIdsQuery` | `query/scaladsl/PersistenceIdsQuery.scala` | 未対応 | std | medium | 同上 |
| `CurrentPersistenceIdsQuery` | `query/scaladsl/CurrentPersistenceIdsQuery.scala` | 未対応 | std | medium | 同上 |
| `PagedPersistenceIdsQuery` | `query/scaladsl/PagedPersistenceIdsQuery.scala` | 未対応 | std | easy | ページネーション版 |
| `CurrentLastSequenceNumberByPersistenceIdQuery` | `query/scaladsl/CurrentLastSequenceNumberByPersistenceIdQuery.scala` | 未対応 | std | easy | |
| `DurableStateStoreQuery[A]` | `query/scaladsl/DurableStateStoreQuery.scala` | 未対応 | std | medium | Source 依存 |
| `DurableStateStorePagedPersistenceIdsQuery` | `query/scaladsl/DurableStateStorePagedPersistenceIdsQuery.scala` | 未対応 | std | easy | |

#### 13f. typed クエリトレイト

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `EventsBySliceQuery` | `query/typed/scaladsl/EventsBySliceQuery.scala` | 未対応 | std | hard | スライスベースのパーティショニング |
| `CurrentEventsBySliceQuery` | `query/typed/scaladsl/CurrentEventsBySliceQuery.scala` | 未対応 | std | hard | 同上 |
| `EventTimestampQuery` | `query/typed/scaladsl/EventTimestampQuery.scala` | 未対応 | std | easy | |
| `LoadEventQuery` | `query/typed/scaladsl/LoadEventQuery.scala` | 未対応 | std | easy | |
| `DurableStateStoreBySliceQuery` | `query/typed/scaladsl/DurableStateStoreBySliceQuery.scala` | 未対応 | std | hard | スライスベース |

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- `RecoveryCompleted` シグナル型 → core（既存コールバックを型化）
- `SnapshotOffer` シグナル型 → core
- `NoSnapshotStore` → core（何もしない SnapshotStore 実装）
- `MaxUnconfirmedMessagesExceededException` → core（PersistenceError バリアント追加）
- `GetObjectResult[A]` → core（構造体追加）
- `DeleteRevisionException` → core（DurableStateException バリアント追加）
- `StashOverflowStrategyConfigurator` → core
- `ReplicaId` → core（newtype）
- `EventRejectedException` → core
- オフセット型（`Sequence`, `NoOffset`, `TimeBasedUUID`）→ core
- `DurableStateChange` 変更型（`UpdatedDurableState`, `DeletedDurableState`）→ core
- `ReadJournal` マーカートレイト → core
- typed シグナル型（`RecoveryCompleted`, `RecoveryFailed` 等の個別型）→ core/typed

### Phase 2: easy（単純な新規実装）

- `PersistenceSettings` → core
- `PersistenceId` → core（構造化 ID）
- `RuntimePluginConfig` → core
- `ReplicationId` → core
- `Offset` 基底型 + `TimestampOffset` → core
- `EventEnvelope` → core
- `DurableStateSignal` ファミリー → core/typed
- `Recovery`(typed)、`SnapshotSelectionCriteria`(typed) → core/typed（classic 版のラッパー）
- typed 版 `EventAdapter[E,P]`, `EventSeq[A]` → core/typed
- `SnapshotAdapter[State]` → core/typed
- `Counter`, `LwwTime` CRDT → core

### Phase 3: medium（中程度の実装工数）

- `AtomicWrite` + Journal trait 拡張 → core（バッチ書き込みの原子性保証）
- `MessageSerializer`, `SnapshotSerializer` → core（serde 統合）
- `RetentionCriteria` / `SnapshotCountRetentionCriteria` → core/typed
- `OpCrdt` trait → core
- `PublishedEvent` → core/typed
- `ReadJournalProvider`, `PersistenceQuery` Extension → core
- `LocalSnapshotStore` → std（ファイルシステム依存）
- クエリトレイト群 → std（streams モジュールとの統合が必要）

### Phase 4: hard（アーキテクチャ変更を伴う）

以下は core 層の大幅な変更または新規レイヤーの追加を伴う：

- **`EventSourcedBehavior[C,E,S]`** → core/typed（typed 層の基盤設計が必要。`Behavior<M>` との統合、Effect 体系の設計が前提）
- **`Effect` / `EffectBuilder` / `ReplyEffect`**（ESB + DSB 両方）→ core/typed（Effect 体系全体の設計。persist/none/unhandled/stop/stash/reply のチェーン API）
- **`DurableStateBehavior[C,S]`** → core/typed（EventSourcedBehavior と基盤を共有）
- **`ReplicatedEventSourcing` / `ReplicationContext`** → core/typed（マルチレプリカ対応のイベントソーシング基盤）
- **`ORSet[A]`** CRDT → core（delta-CRDT の実装は複雑）
- **`EventSourcedProducerQueue`** → core/typed（永続化ベースの信頼性あるデリバリ）
- **スライスベースクエリ**（`EventsBySliceQuery` 等）→ std（スライスパーティショニングの設計が必要）

### 対象外（n/a）

- **PersistentFSM ファミリー**（`PersistentFSM`, `PersistentFSMBase`, `FSMState`, `StateChangeEvent`）: Pekko で非推奨。fraktor-rs には最小限の `PersistentFsm` トレイトが存在。FSM DSL の移植は不要
- **Java API** 固有型（`AbstractPersistentActor`, `AbstractPersistentActorWithTimers`, `AbstractPersistentActorWithAtLeastOnceDelivery`, 各種 Builder クラス）: Rust には不要
- **javadsl パッケージ**: Rust には不要
- **LevelDB 実装**: Pekko で非推奨。独自のストレージバックエンド設計を推奨
- **PersistentFSMMigration**: Akka → Pekko マイグレーション用。不要

---

## まとめ

**全体カバレッジ: classic 層は主要機能をカバー済み（70%）、typed・query 層は手薄**

fraktor-rs の persistence モジュールは Pekko persistence classic（untyped）の核心機能を堅実にカバーしている。
ジャーナル、スナップショット、イベントアダプタ、少なくとも1回配送、永続化状態ストアの基本的な trait とメッセージプロトコルはすべて実装済みであり、InMemory 実装によるテスタビリティも確保されている。

**即座に価値を提供できる未実装機能（Phase 1〜2）:**
- `PersistenceId`（構造化された永続化 ID）: EntityType + EntityId の分離により、クエリ機能の前提条件となる
- `RecoveryCompleted` / `SnapshotOffer` シグナル型: コールバックの型安全化
- `RetentionCriteria`: スナップショット後のイベント・スナップショット自動削除はプロダクション運用で必須
- オフセット型 + `EventEnvelope`: クエリ層の基盤データ型

**実用上の主要ギャップ（Phase 3〜4）:**
- **persistence-typed**（`EventSourcedBehavior` + `Effect` 体系）: Pekko の推奨 API。classic の `PersistentActor` よりも型安全で宣言的。fraktor-rs の `Behavior<M>` との統合設計が必要
- **persistence-query**: CQRS の読み取り側が完全に欠落。streams モジュールとの統合が前提
- **レプリケーション / CRDT**: マルチデータセンター対応は先進的な機能

**YAGNI 観点での省略推奨:**
- PersistentFSM DSL の完全移植（Pekko で非推奨、EventSourcedBehavior が後継）
- Java API / javadsl パッケージ（Rust には不要）
- LevelDB 実装（非推奨、独自ストレージバックエンド設計を推奨）
- `StashOverflowStrategyConfigurator`（設定からの解決は Rust の型システムで代替可能）
- `AtomicWrite`（現在の Journal trait の `write_messages(&[PersistentRepr])` で十分な場合）
