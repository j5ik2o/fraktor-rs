# persistence モジュール ギャップ分析

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（公開API + 主要内部型） | 約50型 |
| fraktor-rs 公開型数 | 約42型 |
| カバレッジ（機能カテゴリ単位） | 6/10 (60%) |
| 主要ギャップ数 | 13 |

> 注: fraktor-rsのpersistenceモジュールはPekkoのコアイベントソーシング機能の大部分をカバーしている。主なギャップはEvent Adapter（イベント変換）、PersistentFSM（状態マシン）、DurableState（永続化状態ストア）の3つの機能群に集中している。

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
| `PersistentRepr.adapters` | `Persistent.scala` | 未対応 | medium | EventAdapters統合（EventAdapter前提） |
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
| `WriteJournalBase` | `journal/WriteJournalBase.scala` | 未対応（暗黙的） | n/a | 内部ユーティリティ。EventAdapter前提 |

### 6. イベントアダプター

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `EventAdapter` trait | `journal/EventAdapter.scala` | 未対応 | medium | 双方向イベント変換。スキーマ進化に必須 |
| `WriteEventAdapter` trait | `journal/EventAdapter.scala` | 未対応 | easy | ドメイン→ジャーナルへの変換 |
| `ReadEventAdapter` trait | `journal/EventAdapter.scala` | 未対応 | easy | ジャーナル→ドメインへの変換 |
| `EventSeq` (sealed) | `journal/EventAdapter.scala` | 未対応 | easy | 読み取りアダプター結果（1:N変換対応） |
| `IdentityEventAdapter` | `journal/EventAdapter.scala` | 未対応 | trivial | 無変換アダプター |
| `Tagged` case class | `journal/Tagged.scala` | 未対応 | easy | イベントへのタグ付け（クエリ用） |
| `EventAdapters` class | `journal/EventAdapters.scala` | 未対応 | medium | アダプターレジストリ（型→アダプターマッピング） |

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
| `Persistence.adaptersFor` | `Persistence.scala` | 未対応 | medium | EventAdapters前提 |
| `PersistenceIdentity` trait | `Persistence.scala` | `Eventsourced::persistence_id()` に統合 | - | 統合済み |
| `PersistenceStash` trait | `Persistence.scala` | persist()メソッド内で暗黙的に処理 | - | 統合済み |
| `RuntimePluginConfig` trait | `Persistence.scala` | 未対応 | easy | ランタイムプラグイン設定 |

### 10. Durable State（永続化状態ストア）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `DurableStateStore[A]` trait | `state/scaladsl/DurableStateStore.scala` | 未対応 | medium | CRUD型の状態永続化。Event Sourcingの代替 |
| `DurableStateUpdateStore[A]` | `state/scaladsl/DurableStateUpdateStore.scala` | 未対応 | medium | 更新セマンティクス付きストア |
| `DurableStateStoreProvider` | `state/DurableStateStoreProvider.scala` | 未対応 | easy | プラグインプロバイダー |
| `DurableStateStoreRegistry` | `state/DurableStateStoreRegistry.scala` | 未対応 | medium | ストアレジストリ Extension |
| `DurableStateException` | `state/exception/DurableStateException.scala` | 未対応 | trivial | エラー型 |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- `PersistentRepr.deleted` - 論理削除フラグの追加
- `deleteSnapshot(seqNr)` - 単一スナップショット削除（criteriaベースで委譲可能）
- `IdentityEventAdapter` - 無変換アダプター（EventAdapter体系の前提）
- `DurableStateException` - エラー型の追加

### Phase 2: easy（単純な新規実装）
- `WriteEventAdapter` trait - ドメイン→ジャーナル変換の1メソッドtrait
- `ReadEventAdapter` trait - ジャーナル→ドメイン変換の1メソッドtrait
- `EventSeq` enum - 読み取りアダプター結果の表現
- `Tagged` struct - イベントへのタグ付け（query対応用）
- `StashOverflowStrategy` enum - スタッシュ溢れ戦略
- `RecoveryTimedOut` エラー型 - リカバリタイムアウト
- `UnconfirmedWarning` - リデリバリ警告メッセージ
- `persistAllAsync` - persist_allのunfenced版
- `PersistentRepr.sender` - 送信元ActorRefの追加
- `RuntimePluginConfig` trait - ランタイム設定
- `FSMState` trait - FSM状態マーカー
- `DurableStateStoreProvider` - プラグインプロバイダー

### Phase 3: medium（中程度の実装工数）
- `EventAdapter` trait（= WriteEventAdapter + ReadEventAdapter統合）
- `EventAdapters` レジストリ - 型→アダプターマッピング
- `defer` / `deferAsync` - persistハンドラ完了後の遅延実行
- `DurableStateStore[A]` trait - CRUD型状態永続化
- `DurableStateUpdateStore[A]` trait - 更新セマンティクス付き
- `DurableStateStoreRegistry` Extension
- `PersistentRepr.adapters` - EventAdaptersとの統合

### Phase 4: hard（アーキテクチャ変更を伴う）
- `PersistentFSM` - イベントソーシング + FSMの統合。3型パラメータ（State, Data, Event）の型安全なFSM
- `PersistentFSMBase` - FSM基盤（when, onTransition, initialize等のDSL）

### 対象外（n/a）
- `AbstractPersistentActor` - Java API。Rustでは不要
- `AbstractPersistentFSMBase` - Java API
- `WriteJournalBase` - 内部ユーティリティ。EventAdapter実装時に自然に導入
- `MessageSerializer` / `SnapshotSerializer` - JVM Serialization固有
- `AsyncWriteProxy` - JVM固有のプロキシ機構

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
