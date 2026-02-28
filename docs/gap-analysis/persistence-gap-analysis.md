# persistence モジュール ギャップ分析

> 分析日: 2026-02-27（前回: 2026-02-24）
> 対象: `modules/persistence/src/` vs `references/pekko/persistence/src/`

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（公開API + 主要内部型） | 約50型 |
| fraktor-rs 公開型数 | 約42型 |
| カバレッジ（機能カテゴリ単位） | 8/10 (80%)（前回 6/10 → 改善） |
| 主要ギャップ数 | 5（前回13 → 8件削減） |

> 注: fraktor-rsのpersistenceモジュールはPekkoのコアイベントソーシング機能の大部分をカバーしている。前回分析から EventAdapter（イベント変換）と DurableState（永続化状態ストア）が新たに実装された。

### 前回分析からの変更

以下の機能が新たに実装済みとなった：
- `WriteEventAdapter` trait → 完全実装
- `ReadEventAdapter` trait → 完全実装
- `EventAdapters` レジストリ → 完全実装（TypeIdベースのアダプター解決）
- `IdentityEventAdapter` → 完全実装
- `DurableStateStore<A>` trait → 完全実装（get_object, upsert_object, delete_object）
- `DurableStateUpdateStore` → 完全実装
- `DurableStateStoreProvider` → 完全実装
- `DurableStateStoreRegistry` → 完全実装
- `DurableStateException` → 完全実装

## カテゴリ別ギャップ

### 1. コアPersistent Actor

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `PersistentActor` trait | `PersistentActor.scala` | `PersistentActor<TB>` trait | - | 実装済み |
| `Eventsourced` trait | `Eventsourced.scala` | `Eventsourced<TB>` trait | - | 実装済み |
| `persist[A](event)(handler)` | `PersistentActor.scala` | `persist(ctx, event, handler)` | - | 実装済み（stashing=true） |
| `persistAsync[A](event)(handler)` | `PersistentActor.scala` | `persist_unfenced(ctx, event, handler)` | - | 別名で実装済み（stashing=false） |
| `persistAll[A](events)(handler)` | `PersistentActor.scala` | `persist_all(ctx, events, handler)` | - | 実装済み |
| `persistAllAsync[A](events)(handler)` | `PersistentActor.scala` | 未対応 | easy | persist_allのunfenced版が不在 |
| `defer[A](event)(handler)` | `PersistentActor.scala` | 未対応 | medium | persistハンドラ完了後のアクション遅延実行 |
| `deferAsync[A](event)(handler)` | `PersistentActor.scala` | 未対応 | medium | deferの非同期版 |
| `deleteMessages(toSeqNr)` | `PersistentActor.scala` | `delete_messages(ctx, to_sequence_nr)` | - | 実装済み |
| `saveSnapshot(snapshot)` | `PersistentActor.scala` | `save_snapshot(ctx, snapshot)` | - | 実装済み |
| `deleteSnapshot(seqNr)` | `PersistentActor.scala` | 未対応（deleteSnapshotsはあり） | trivial | 単一スナップショット削除 |
| `deleteSnapshots(criteria)` | `PersistentActor.scala` | `delete_snapshots(ctx, criteria)` | - | 実装済み |
| `AbstractPersistentActor` | `PersistentActor.scala` | 未対応 | n/a | Java API。Rustでは不要 |

### 2. リカバリ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Recovery` case class | `PersistentActor.scala` | `Recovery` struct | - | 実装済み |
| `Recovery.none` | `PersistentActor.scala` | `Recovery::none()` | - | 実装済み |
| `RecoveryCompleted` message | `PersistentActor.scala` | `on_recovery_completed()` callback | - | コールバックパターンで実装済み |
| `PersistenceRecovery` trait | `Persistence.scala` | `Eventsourced::recovery()` に統合 | - | 統合済み |
| `StashOverflowStrategy` (sealed) | `PersistentActor.scala` | 未対応 | easy | スタッシュ溢れ時の戦略（DeadLetter/Throw/Reply） |
| `RecoveryTimedOut` exception | `PersistentActor.scala` | 未対応 | easy | リカバリタイムアウトのエラー型 |

### 3. 永続化メッセージ表現

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `PersistentRepr` trait | `Persistent.scala` | `PersistentRepr` struct | - | 実装済み |
| `PersistentRepr.payload` | `Persistent.scala` | `PersistentRepr::payload()` | - | 実装済み |
| `PersistentRepr.persistenceId` | `Persistent.scala` | `PersistentRepr::persistence_id()` | - | 実装済み |
| `PersistentRepr.sequenceNr` | `Persistent.scala` | `PersistentRepr::sequence_nr()` | - | 実装済み |
| `PersistentRepr.manifest` | `Persistent.scala` | `PersistentRepr::manifest()` | - | 実装済み |
| `PersistentRepr.metadata` | `Persistent.scala` | `PersistentRepr::metadata()` | - | 実装済み |
| `PersistentRepr.timestamp` | `Persistent.scala` | `PersistentRepr::timestamp()` | - | 実装済み |
| `PersistentRepr.deleted` | `Persistent.scala` | 未対応 | trivial | 論理削除フラグ |
| `PersistentRepr.sender` | `Persistent.scala` | 未対応 | easy | 送信元ActorRef |
| `AtomicWrite` case class | `Persistent.scala` | `JournalMessage::WriteMessages` | - | メッセージvariantとして実装済み |

### 4. スナップショット管理

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `SnapshotMetadata` | `SnapshotProtocol.scala` | `SnapshotMetadata` | - | 実装済み |
| `SelectedSnapshot` | `SnapshotProtocol.scala` | `Snapshot` | - | 別名で実装済み |
| `SnapshotSelectionCriteria` | `SnapshotProtocol.scala` | `SnapshotSelectionCriteria` | - | 実装済み |
| `SnapshotOffer` message | `SnapshotProtocol.scala` | `receive_snapshot()` callback | - | コールバックパターンで実装済み |
| `SaveSnapshotSuccess` | `SnapshotProtocol.scala` | `SnapshotResponse::SaveSnapshotSuccess` | - | 実装済み |
| `SaveSnapshotFailure` | `SnapshotProtocol.scala` | `SnapshotResponse::SaveSnapshotFailure` | - | 実装済み |
| `DeleteSnapshotSuccess` | `SnapshotProtocol.scala` | `SnapshotResponse::DeleteSnapshotSuccess` | - | 実装済み |
| `DeleteSnapshotsSuccess` | `SnapshotProtocol.scala` | `SnapshotResponse::DeleteSnapshotsSuccess` | - | 実装済み |
| `DeleteSnapshotFailure` | `SnapshotProtocol.scala` | `SnapshotResponse::DeleteSnapshotFailure` | - | 実装済み |
| `DeleteSnapshotsFailure` | `SnapshotProtocol.scala` | `SnapshotResponse::DeleteSnapshotsFailure` | - | 実装済み |
| `Snapshotter` trait | `Snapshotter.scala` | `PersistentActor` に統合 | - | 統合済み |
| `SnapshotStore` trait | `snapshot/SnapshotStore.scala` | `SnapshotStore` trait | - | 実装済み |

### 5. ジャーナルプラグイン

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `AsyncWriteJournal` trait | `journal/AsyncWriteJournal.scala` | `Journal` trait | - | 統合実装済み |
| `AsyncRecovery` trait | `journal/AsyncRecovery.scala` | `Journal` trait に統合 | - | replay_messages, highest_sequence_nrとして統合 |
| `asyncWriteMessages` | `AsyncWriteJournal.scala` | `Journal::write_messages` | - | 実装済み |
| `asyncDeleteMessagesTo` | `AsyncWriteJournal.scala` | `Journal::delete_messages_to` | - | 実装済み |
| `asyncReplayMessages` | `AsyncRecovery.scala` | `Journal::replay_messages` | - | 実装済み |
| `asyncReadHighestSequenceNr` | `AsyncRecovery.scala` | `Journal::highest_sequence_nr` | - | 実装済み |

### 6. イベントアダプター

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `EventAdapter` trait | `journal/EventAdapter.scala` | `WriteEventAdapter` + `ReadEventAdapter` | - | **実装済み**（2 trait に分離） |
| `WriteEventAdapter` trait | `journal/EventAdapter.scala` | `WriteEventAdapter` trait | - | **実装済み** |
| `ReadEventAdapter` trait | `journal/EventAdapter.scala` | `ReadEventAdapter` trait | - | **実装済み** |
| `EventSeq` (sealed) | `journal/EventAdapter.scala` | `EventSeq` | - | **実装済み** |
| `IdentityEventAdapter` | `journal/EventAdapter.scala` | `IdentityEventAdapter` | - | **実装済み** |
| `Tagged` case class | `journal/Tagged.scala` | 未対応 | easy | イベントへのタグ付け（クエリ用） |
| `EventAdapters` class | `journal/EventAdapters.scala` | `EventAdapters` struct | - | **実装済み**（TypeIdベースのアダプター解決） |

### 7. At-Least-Once Delivery

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `AtLeastOnceDelivery` trait | `AtLeastOnceDelivery.scala` | `AtLeastOnceDeliveryGeneric<TB>` | - | 実装済み |
| `deliver(destination)(f)` | `AtLeastOnceDelivery.scala` | `deliver(destination, sender, timestamp, build)` | - | 実装済み |
| `confirmDelivery(id)` | `AtLeastOnceDelivery.scala` | `confirm_delivery(delivery_id)` | - | 実装済み |
| `numberOfUnconfirmed` | `AtLeastOnceDelivery.scala` | `number_of_unconfirmed()` | - | 実装済み |
| `getDeliverySnapshot` | `AtLeastOnceDelivery.scala` | `get_delivery_snapshot()` | - | 実装済み |
| `setDeliverySnapshot` | `AtLeastOnceDelivery.scala` | `set_delivery_snapshot()` | - | 実装済み |
| `AtLeastOnceDeliverySnapshot` | `AtLeastOnceDelivery.scala` | `AtLeastOnceDeliverySnapshot<TB>` | - | 実装済み |
| `UnconfirmedDelivery` | `AtLeastOnceDelivery.scala` | `UnconfirmedDelivery<TB>` | - | 実装済み |
| `UnconfirmedWarning` message | `AtLeastOnceDelivery.scala` | 未対応 | easy | リデリバリ試行回数超過警告 |
| `MaxUnconfirmedMessagesExceededException` | `AtLeastOnceDelivery.scala` | `can_accept_more()` メソッドで代替 | - | パターンは異なるが機能的に対応 |

### 8. Persistent FSM（状態マシン）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `PersistentFSM[S, D, E]` trait | `fsm/PersistentFSM.scala` | 未対応 | hard | イベントソーシング + FSM。3型パラメータ |
| `PersistentFSMBase[S, D, E]` | `fsm/PersistentFSMBase.scala` | 未対応 | hard | FSM基盤（when, onTransition等） |
| `FSMState` trait | `fsm/PersistentFSM.scala` | 未対応 | easy | FSM状態マーカートレイト |
| `LoggingPersistentFSM` | `fsm/PersistentFSMBase.scala` | 未対応 | easy | ロギング付きFSM |

### 9. Persistence Extension・設定

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Persistence` Extension | `Persistence.scala` | `PersistenceExtensionGeneric<TB>` | - | 実装済み |
| `Persistence.journalFor` | `Persistence.scala` | `journal_actor_ref()` | - | 実装済み |
| `Persistence.snapshotStoreFor` | `Persistence.scala` | `snapshot_actor_ref()` | - | 実装済み |
| `Persistence.adaptersFor` | `Persistence.scala` | `EventAdapters` 経由 | - | **実装済み** |
| `PersistenceIdentity` trait | `Persistence.scala` | `Eventsourced::persistence_id()` に統合 | - | 統合済み |
| `PersistenceStash` trait | `Persistence.scala` | persist()メソッド内で暗黙的に処理 | - | 統合済み |

### 10. Durable State（永続化状態ストア）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `DurableStateStore[A]` trait | `state/scaladsl/DurableStateStore.scala` | `DurableStateStore<A>` trait | - | **実装済み** |
| `DurableStateUpdateStore[A]` | `state/scaladsl/DurableStateUpdateStore.scala` | `DurableStateUpdateStore` | - | **実装済み** |
| `DurableStateStoreProvider` | `state/DurableStateStoreProvider.scala` | `DurableStateStoreProvider` | - | **実装済み** |
| `DurableStateStoreRegistry` | `state/DurableStateStoreRegistry.scala` | `DurableStateStoreRegistry` | - | **実装済み** |
| `DurableStateException` | `state/exception/DurableStateException.scala` | `DurableStateException` | - | **実装済み** |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- `PersistentRepr.deleted` - 論理削除フラグの追加
- `deleteSnapshot(seqNr)` - 単一スナップショット削除（criteriaベースで委譲可能）

### Phase 2: easy（単純な新規実装）
- `Tagged` struct - イベントへのタグ付け（query対応用）
- `StashOverflowStrategy` enum - スタッシュ溢れ戦略
- `RecoveryTimedOut` エラー型 - リカバリタイムアウト
- `UnconfirmedWarning` - リデリバリ警告メッセージ
- `persistAllAsync` - persist_allのunfenced版
- `PersistentRepr.sender` - 送信元ActorRefの追加
- `FSMState` trait - FSM状態マーカー

### Phase 3: medium（中程度の実装工数）
- `defer` / `deferAsync` - persistハンドラ完了後の遅延実行

### Phase 4: hard（アーキテクチャ変更を伴う）
- `PersistentFSM` - イベントソーシング + FSMの統合。3型パラメータ（State, Data, Event）の型安全なFSM
- `PersistentFSMBase` - FSM基盤（when, onTransition, initialize等のDSL）

### 対象外（n/a）
- `AbstractPersistentActor` - Java API。Rustでは不要
- `AbstractPersistentFSMBase` - Java API
- `WriteJournalBase` - 内部ユーティリティ
- `MessageSerializer` / `SnapshotSerializer` - JVM Serialization固有
- `AsyncWriteProxy` - JVM固有のプロキシ機構
- `RuntimePluginConfig` - JVM固有のプラグイン設定

## 補足: 設計差異

| 観点 | Pekko | fraktor-rs | 備考 |
|------|-------|-----------|------|
| persistAsync | `persistAsync` | `persist_unfenced` | "unfenced"はTokio async との混同回避 |
| RecoveryCompleted | メッセージとして受信 | `on_recovery_completed()` callback | コールバックパターンで統合 |
| SnapshotOffer | メッセージとして受信 | `receive_snapshot()` callback | コールバックパターンで統合 |
| AtomicWrite | 専用case class | `JournalMessage::WriteMessages` variant | enum variantとして統合 |
| Snapshotter | 独立trait | `PersistentActor` に統合 | Rustでは分離不要 |
| PersistenceIdentity | 独立trait | `Eventsourced` に統合 | trait継承よりフラット構成 |
| Journal/AsyncRecovery | 2つの独立trait | `Journal` trait 1つに統合 | GATベースで統合 |
| InMemoryJournal | テストキット（別モジュール） | `InMemoryJournal`（persistence内） | テスト用途を同梱 |
| EventAdapter | 1つのtrait（双方向） | `WriteEventAdapter` + `ReadEventAdapter` の2 trait | 責務分離 |
| DurableState | 別モジュール | persistence内に統合 | モジュール統合 |

---

## 総評

fraktor-rs の persistence モジュールは前回分析から大幅に改善され、カバレッジが **60% → 80%** に向上した。特に EventAdapter 基盤（Write/Read アダプター、EventAdapters レジストリ、IdentityEventAdapter）と DurableState 全体（Store, UpdateStore, Provider, Registry, Exception）が完全に実装された。

残るギャップは以下に集中：
1. **PersistentFSM**（hard）— イベントソーシング + 有限状態マシンの統合
2. **defer / deferAsync**（medium）— persist ハンドラ完了後の遅延実行
3. **細かな補助型**（easy/trivial）— Tagged, StashOverflowStrategy, PersistentRepr.deleted/sender 等

コア機能（persist, persistAsync, persistAll, Recovery, Snapshot, Journal, EventAdapter, DurableState, AtLeastOnceDelivery）は完全にカバーされており、イベントソーシングの基本的なワークフローは fraktor-rs で実現可能。

## 次の推奨プラン（全体優先度）

当面は `actor` と `streams` の互換性強化を主目的とし、`cluster` の SBR/クラスタ再配置周りは defer する。

- 第1優先（安定性）: `persistence` は既存のコア機能を崩さず、easy 領域の補完を最小で実施
  - `Tagged`, `StashOverflowStrategy`, `RecoveryTimedOut`, `persistAllAsync`, `PersistentRepr.sender` 等
- 第2優先（連携）: `actor` / `streams` 側が進んだタイミングで、`persistence` の運用面影響が出る項目（イベント再送・defer 系）との整合を再確認
- 第3優先: `PersistentFSM` は現在は保留。クラスタ運用条件・CQRS/ES 方針が確定した時点で導入可否を再検討
