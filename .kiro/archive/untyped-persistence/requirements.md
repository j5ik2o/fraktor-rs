# 要件ドキュメント

## 導入

本ドキュメントは、fraktor-persistence-rs クレートの Untyped 永続化モジュールに対する要件を定義する。既存の `modules/persistence/src` 以下のソースファイルは全て削除し、完全に再設計・再実装を行う。

本モジュールは以下の設計方針に従う:

1. **メッセージパッシングアーキテクチャ**: PersistentActor は Journal/SnapshotStore の API を直接呼び出さず、メッセージパッシングを通じて通信する。これは Pekko の Eventsourced 実装と同様のパターンである。

2. **GATsパターン + Future型**: `async fn` を使用せず、trait内でGeneric Associated Types（GATs）を使用してFuture型を関連型として定義する。同期実装では `core::future::Ready<T>` を返し、非同期実装では独自Future型を返す。

3. **`core::future::Ready<T>`の導入と将来の横展開**: 同期実装（InMemory等）では `core::future::ready()` を使用してFutureを返す。他モジュール（cluster等）への横展開は将来の別 spec で対応する。

4. **Future駆動の責務分離**: Journal/SnapshotStore が返す Future は JournalActor/SnapshotActor が完了まで駆動し、PersistentActor は Future を直接 poll しない。再ポーリングは自己メッセージで行い、Dispatcher を直接呼び出さない。

5. **no_std専用**: `#![no_std]` 環境のみで実装し、`alloc` クレートに依存する。coreモジュールではstdに一切依存しない。std 向け拡張は今回のスコープ外とする。

6. **Pekko仕様互換**: Apache Pekko の永続化セマンティクス（特に Eventsourced パターン）と互換性を持ち、イベントソーシングと CQRS パターンをサポートする。

主要コンポーネント:
- **ストレージ層**: `Journal` trait、`SnapshotStore` trait（GATsパターン）
- **メッセージプロトコル**: `JournalMessage`/`JournalResponse`、`SnapshotMessage`/`SnapshotResponse`
- **アクターラッパー**: `JournalActor`、`SnapshotActor`（ストレージをアクター化）
- **永続化アクター**: `PersistentActorBase`、`Eventsourced` trait、`PersistentActor` trait
- **状態管理**: `PersistentActorState`（状態マシン）、`PendingHandlerInvocation`（ハンドラーキューイング）
- **データ構造**: `PersistentRepr`、`PersistentEnvelope`、`SnapshotMetadata`、`SnapshotSelectionCriteria`、`Recovery`
- **配信保証**: `AtLeastOnceDelivery`
- **拡張**: `PersistenceExtension`
- **テスト用実装**: `InMemoryJournal`、`InMemorySnapshotStore`（`core::future::ready()`使用）

## 要件

### 要件1: GATsパターンによるtrait設計

**目的:** 開発者として `async fn` を使用せずにno_std環境で非同期操作を抽象化し、同期・非同期両方のコンテキストで永続化操作を行いたい。

#### 受け入れ条件
1. Journal trait が定義されたとき、永続化モジュールは Generic Associated Types（GATs）を使用してFuture型を関連型として定義しなければならない
2. SnapshotStore trait が定義されたとき、永続化モジュールは Generic Associated Types（GATs）を使用してFuture型を関連型として定義しなければならない
3. すべてのGATs Future型は `Future<Output = Result<T, E>> + Send + 'a` を満たさなければならない
4. Journal trait のGATs定義は `type WriteFuture<'a>: Future<Output = Result<(), JournalError>> + Send + 'a where Self: 'a` のパターンに従わなければならない
5. Journal trait のGATs定義は `type ReplayFuture<'a>: Future<Output = Result<Vec<PersistentRepr>, JournalError>> + Send + 'a where Self: 'a` のパターンに従わなければならない
6. SnapshotStore trait のGATs定義は `type SaveFuture<'a>: Future<Output = Result<(), SnapshotError>> + Send + 'a where Self: 'a` のパターンに従わなければならない
7. SnapshotStore trait のGATs定義は `type LoadFuture<'a>: Future<Output = Result<Option<Snapshot>, SnapshotError>> + Send + 'a where Self: 'a` のパターンに従わなければならない
8. すべてのGATs Future型はライフタイムパラメータ `'a` を持ち、selfの借用に依存しなければならない
9. GATsパターンは `modules/cluster/src/core/activation_executor.rs` および `modules/cluster/src/core/activation_storage.rs` の既存実装と一貫性を持たなければならない

### 要件2: `core::future::Ready<T>`による同期実装

**目的:** 開発者として InMemory 実装などの同期的な永続化操作を `core::future::Ready<T>` で効率的に提供し、テスト環境での使用を容易にしたい。

#### 受け入れ条件
1. InMemoryJournal が実装されたとき、すべてのFuture関連型は `core::future::Ready<Result<T, E>>` を返さなければならない
2. InMemorySnapshotStore が実装されたとき、すべてのFuture関連型は `core::future::Ready<Result<T, E>>` を返さなければならない
3. `core::future::ready()` 関数が呼び出されたとき、永続化モジュールは即座に完了するFutureを生成しなければならない
4. `core::future::Ready<T>` を使用した実装は `.await` で非同期的に使用可能でなければならない
5. `core::future::Ready<T>` を使用した実装は `poll` メソッドで同期的に結果を取得可能でなければならない
6. すべての同期実装は no_std 環境で動作しなければならない

### 要件3: 他モジュールへの`core::future::Ready<T>`横展開（スコープ外）

**目的:** 開発者として cluster モジュール等のGATsパターンを使用するtrait実装においても、同期版（InMemory/Noop等）で `core::future::Ready<T>` を使用し、プロジェクト全体で一貫したパターンを適用したい。

**注記:** 本要件は **将来の別 spec（cluster-ready-integration）で対応** とし、本 spec のスコープ外とする。理由:
- persistence モジュールで `core::future::Ready<T>` パターンを検証後、cluster に適用すべき
- 本 spec は persistence モジュールの GATs 化に集中
- 既存 cluster コードへの影響を最小化

#### 受け入れ条件（将来の別 spec で実施）
1. ~~clusterモジュールでGATsパターンを使用するtraitのInMemory/Noop実装が存在する場合、`core::future::Ready<Result<T, E>>` を返すようリファクタリングしなければならない~~
2. ~~`ActivationStorage` trait の同期実装が追加される場合、すべてのFuture関連型は `core::future::Ready<Result<T, E>>` を返さなければならない~~
3. ~~`PlacementLock` trait の同期実装が追加される場合、すべてのFuture関連型は `core::future::Ready<Result<T, E>>` を返さなければならない~~
4. ~~`ActivationExecutor` trait の同期実装が追加される場合、すべてのFuture関連型は `core::future::Ready<Result<T, E>>` を返さなければならない~~
5. ~~横展開対象のすべての実装は no_std 環境で動作しなければならない~~
6. ~~横展開により既存のテストが破壊されてはならない~~

**本 spec での対応:**
- persistence モジュールで `core::future::Ready<T>` パターンを確立
- 将来の cluster 横展開のための設計パターンを文書化

### 要件4: Journal trait の設計と実装

**目的:** 開発者として Pekko 互換のイベントジャーナルを抽象化し、GATsパターンで様々なストレージバックエンドに対応できるようにしたい。

#### 受け入れ条件
1. `Journal::write_messages` が呼び出されたとき、Journal は `Self::WriteFuture<'_>` を返さなければならない
2. `Journal::write_messages` の戻り値のFutureが完了したとき、Journal はメッセージを正しく永続化しなければならない
3. `Journal::write_messages` でシーケンス番号が不連続な場合、Journal は `JournalError::SequenceMismatch` を返さなければならない
4. `Journal::replay_messages` が呼び出されたとき、Journal は `Self::ReplayFuture<'_>` を返さなければならない
5. `Journal::replay_messages` の戻り値のFutureが完了したとき、Journal は指定された範囲のメッセージを正しく返さなければならない
6. `Journal::replay_messages` で `max` パラメータが指定されたとき、Journal は最大件数を超えないメッセージを返さなければならない
7. `Journal::delete_messages_to` が呼び出されたとき、Journal は `Self::DeleteFuture<'_>` を返さなければならない
8. `Journal::delete_messages_to` の戻り値のFutureが完了したとき、Journal は指定されたシーケンス番号以下のメッセージを削除しなければならない
9. `Journal::highest_sequence_nr` が呼び出されたとき、Journal は `Self::HighestSeqNrFuture<'_>` を返さなければならない
10. `Journal::highest_sequence_nr` の戻り値のFutureが完了したとき、Journal は格納された最大シーケンス番号を返さなければならない
11. 存在しない persistence_id に対して操作が行われたとき、Journal はエラーなく処理を完了しなければならない

### 要件5: SnapshotStore trait の設計と実装

**目的:** 開発者として Pekko 互換のスナップショットストアを抽象化し、GATsパターンで様々なストレージバックエンドに対応できるようにしたい。

#### 受け入れ条件
1. `SnapshotStore::save_snapshot` が呼び出されたとき、SnapshotStore は `Self::SaveFuture<'_>` を返さなければならない
2. `SnapshotStore::save_snapshot` の戻り値のFutureが完了したとき、SnapshotStore はスナップショットを正しく保存しなければならない
3. `SnapshotStore::load_snapshot` が呼び出されたとき、SnapshotStore は `Self::LoadFuture<'_>` を返さなければならない
4. `SnapshotStore::load_snapshot` の戻り値のFutureが完了したとき、SnapshotStore は条件に一致する最新のスナップショットを返さなければならない
5. `SnapshotStore::load_snapshot` で条件に一致するスナップショットがない場合、SnapshotStore は `None` を返さなければならない
6. `SnapshotStore::delete_snapshot` が呼び出されたとき、SnapshotStore は `Self::DeleteOneFuture<'_>` を返さなければならない
7. `SnapshotStore::delete_snapshot` の戻り値のFutureが完了したとき、SnapshotStore は指定されたスナップショットを削除しなければならない
8. `SnapshotStore::delete_snapshots` が呼び出されたとき、SnapshotStore は `Self::DeleteManyFuture<'_>` を返さなければならない
9. `SnapshotStore::delete_snapshots` の戻り値のFutureが完了したとき、SnapshotStore は条件に一致するすべてのスナップショットを削除しなければならない

### 要件6: PersistentRepr の設計と実装

**目的:** 開発者として永続化イベントを型安全に表現し、メタデータを付与できるようにしたい。

#### 受け入れ条件
1. `PersistentRepr::new` が呼び出されたとき、PersistentRepr は persistence_id, sequence_nr, payload を保持しなければならない
2. `PersistentRepr::downcast_ref` が呼び出されたとき、PersistentRepr は正しい型へのダウンキャストを行わなければならない
3. 不正な型でダウンキャストが試みられたとき、PersistentRepr は `None` を返さなければならない
4. `PersistentRepr::with_manifest` が呼び出されたとき、PersistentRepr は manifest を更新した新しいインスタンスを返さなければならない
5. `PersistentRepr::with_metadata` が呼び出されたとき、PersistentRepr は metadata を付与した新しいインスタンスを返さなければならない
6. PersistentRepr は常に `Clone` を実装しなければならない

### 要件7: SnapshotMetadata と SnapshotSelectionCriteria の設計と実装

**目的:** 開発者としてスナップショットのメタデータを管理し、条件に基づいて選択できるようにしたい。

#### 受け入れ条件
1. `SnapshotMetadata::new` が呼び出されたとき、SnapshotMetadata は persistence_id, sequence_nr, timestamp を保持しなければならない
2. `SnapshotMetadata::with_metadata` が呼び出されたとき、SnapshotMetadata は追加メタデータを付与しなければならない
3. 同じ persistence_id, sequence_nr, timestamp を持つ SnapshotMetadata は常に等価でなければならない
4. SnapshotMetadata は常に `Clone` を実装しなければならない
5. `SnapshotSelectionCriteria::matches` が呼び出されたとき、criteria は metadata の sequence_nr と timestamp を正しく評価しなければならない
6. `SnapshotSelectionCriteria::limit` が呼び出されたとき、criteria は max_sequence_nr を正しく制限しなければならない
7. `SnapshotSelectionCriteria::none()` が呼び出されたとき、criteria はすべてのスナップショットにマッチしない条件を返さなければならない
8. `SnapshotSelectionCriteria::latest()` が呼び出されたとき、criteria は最新のスナップショットにマッチする条件を返さなければならない

### 要件8: Recovery 設定の設計と実装

**目的:** 開発者として様々なリカバリシナリオに対応し、永続化アクターの起動時の挙動を制御したい。

#### 受け入れ条件
1. `Recovery::default()` が使用されたとき、Recovery は最新のスナップショットから全イベントを再生する設定を返さなければならない
2. `Recovery::none()` が使用されたとき、Recovery はスナップショットとイベントの再生をスキップする設定を返さなければならない
3. `Recovery::new` で `to_sequence_nr` が指定されたとき、Recovery は指定されたシーケンス番号までのイベントのみを再生する設定を保持しなければならない
4. `Recovery::new` で `replay_max` が指定されたとき、Recovery は最大件数を超えるイベントの再生を行わない設定を保持しなければならない
5. `Recovery::from_snapshot` で `SnapshotSelectionCriteria` が指定されたとき、Recovery は指定された条件でスナップショットを選択する設定を保持しなければならない
6. Recovery は常に `Clone` を実装しなければならない

### 要件9: JournalMessage / JournalResponse メッセージプロトコル

**目的:** 開発者として PersistentActor と JournalActor 間のメッセージパッシングを型安全に行い、Pekkoと同様の非同期通信パターンを実現したい。

#### 受け入れ条件
1. `JournalMessage::WriteMessages` が送信されたとき、JournalActor は PersistentRepr のバッチを受け取り、永続化処理を開始しなければならない
2. `JournalMessage::ReplayMessages` が送信されたとき、JournalActor は指定された範囲のメッセージをリプレイしなければならない
3. `JournalMessage::DeleteMessagesTo` が送信されたとき、JournalActor は指定されたシーケンス番号以下のメッセージを削除しなければならない
4. `JournalMessage::GetHighestSequenceNr` が送信されたとき、JournalActor は最大シーケンス番号を取得しなければならない
5. `JournalResponse::WriteMessageSuccess` が返されたとき、PersistentActor は永続化成功を認識しなければならない
6. `JournalResponse::WriteMessageFailure` が返されたとき、PersistentActor は永続化失敗を認識しエラーハンドリングを行わなければならない
7. `JournalResponse::ReplayedMessage` が返されたとき、PersistentActor は個別のイベントを受信しなければならない
8. `JournalResponse::RecoverySuccess` が返されたとき、PersistentActor はリプレイ完了を認識しなければならない
9. `JournalResponse::HighestSequenceNr` が返されたとき、PersistentActor は最大シーケンス番号を受信しなければならない
10. すべての JournalMessage は persistence_id と sender（ActorRef）を含み、さらにシーケンス番号情報（単一の sequence_nr、または from/to の範囲、または to_sequence_nr などの上限）を持たなければならない

### 要件10: SnapshotMessage / SnapshotResponse メッセージプロトコル

**目的:** 開発者として PersistentActor と SnapshotActor 間のメッセージパッシングを型安全に行い、スナップショット操作の非同期通信を実現したい。

#### 受け入れ条件
1. `SnapshotMessage::SaveSnapshot` が送信されたとき、SnapshotActor はスナップショットを保存しなければならない
2. `SnapshotMessage::LoadSnapshot` が送信されたとき、SnapshotActor は条件に一致するスナップショットを検索しなければならない
3. `SnapshotMessage::DeleteSnapshot` が送信されたとき、SnapshotActor は指定されたスナップショットを削除しなければならない
4. `SnapshotMessage::DeleteSnapshots` が送信されたとき、SnapshotActor は条件に一致するスナップショット群を削除しなければならない
5. `SnapshotResponse::SaveSnapshotSuccess` が返されたとき、PersistentActor はスナップショット保存成功を認識しなければならない
6. `SnapshotResponse::SaveSnapshotFailure` が返されたとき、PersistentActor はスナップショット保存失敗を認識しエラーハンドリングを行わなければならない
7. `SnapshotResponse::LoadSnapshotResult` が返されたとき、PersistentActor はスナップショット（またはNone）を受信しなければならない
8. すべての SnapshotMessage は persistence_id, sender（ActorRef）を含まなければならない

### 要件11: JournalActor の設計と実装

**目的:** 開発者として Journal trait をアクターとしてラップし、メッセージパッシングによる非同期的なジャーナル操作を実現したい。

#### 受け入れ条件
1. JournalActor が JournalMessage を受信したとき、内部の Journal 実装を使用して操作を実行しなければならない
2. JournalActor は操作完了後、適切な JournalResponse を送信者に返さなければならない
3. JournalActor は複数の PersistentActor からの並行リクエストを適切に処理しなければならない
4. JournalActor は Journal trait の任意の実装を受け入れなければならない（Generic over Journal）
5. JournalActor はツールボックスパターン（RuntimeToolbox）に従わなければならない
6. JournalActor は no_std 環境で動作しなければならない
7. JournalActor は Journal が返す Future を完了まで駆動し、その結果を JournalResponse として送信しなければならない
8. JournalActor は `Poll::Pending` を正常系として扱い、自己メッセージで再ポーリングをスケジュールしなければならない（Dispatcher を直接呼び出さない、drive_ready 等を使用しない）

### 要件12: SnapshotActor の設計と実装

**目的:** 開発者として SnapshotStore trait をアクターとしてラップし、メッセージパッシングによる非同期的なスナップショット操作を実現したい。

#### 受け入れ条件
1. SnapshotActor が SnapshotMessage を受信したとき、内部の SnapshotStore 実装を使用して操作を実行しなければならない
2. SnapshotActor は操作完了後、適切な SnapshotResponse を送信者に返さなければならない
3. SnapshotActor は複数の PersistentActor からの並行リクエストを適切に処理しなければならない
4. SnapshotActor は SnapshotStore trait の任意の実装を受け入れなければならない（Generic over SnapshotStore）
5. SnapshotActor はツールボックスパターン（RuntimeToolbox）に従わなければならない
6. SnapshotActor は no_std 環境で動作しなければならない
7. SnapshotActor は SnapshotStore が返す Future を完了まで駆動し、その結果を SnapshotResponse として送信しなければならない
8. SnapshotActor は `Poll::Pending` を正常系として扱い、自己メッセージで再ポーリングをスケジュールしなければならない（Dispatcher を直接呼び出さない、drive_ready 等を使用しない）

### 要件13: PersistentActorState（状態マシン）の設計と実装

**目的:** 開発者として PersistentActor のライフサイクル状態を追跡し、各状態に応じた適切な振る舞いを実現したい。

#### 受け入れ条件
1. `PersistentActorState::WaitingRecoveryPermit` 状態で、PersistentActor はリカバリ許可を待機しなければならない
2. `PersistentActorState::RecoveryStarted` 状態で、PersistentActor はスナップショットロードを開始しなければならない
3. `PersistentActorState::Recovering` 状態で、PersistentActor はイベントリプレイを処理しなければならない
4. `PersistentActorState::ProcessingCommands` 状態で、PersistentActor は通常のコマンドを処理できなければならない
5. `PersistentActorState::PersistingEvents` 状態で、PersistentActor はイベント永続化中であることを示し、新しいコマンドをスタッシュしなければならない
6. 状態遷移は `WaitingRecoveryPermit → RecoveryStarted → Recovering → ProcessingCommands ⇄ PersistingEvents` の順序に従わなければならない
7. 不正な状態遷移が試みられたとき、状態マシンは明確なエラーを報告しなければならない

### 要件14: PendingHandlerInvocation（ハンドラーキューイング）の設計と実装

**目的:** 開発者として永続化完了後に呼び出されるハンドラーをキュー管理し、Pekkoと同様の pendingInvocations パターンを実現したい。

#### 受け入れ条件
1. `StashingHandlerInvocation` が作成されたとき、永続化中は後続コマンドをスタッシュしなければならない
2. `AsyncHandlerInvocation` が作成されたとき、永続化中もコマンドをスタッシュしなくてよいことを示さなければならない
3. PersistentActor は `pendingInvocations` キューを保持しなければならない
4. `persist` が呼び出されたとき、PersistentActor は `StashingHandlerInvocation` をキューに追加しなければならない
5. `persistAsync` が呼び出されたとき、PersistentActor は `AsyncHandlerInvocation` をキューに追加しなければならない
6. 永続化成功時、PersistentActor はキューの先頭からハンドラーを取り出し順次実行しなければならない
7. 永続化失敗時、PersistentActor は適切なエラーハンドラーを呼び出さなければならない

### 要件15: Eventsourced trait の設計と実装

**目的:** 開発者として Pekko の Eventsourced と同様のイベントソーシング振る舞いを trait として提供し、再利用可能な基盤を実現したい。

#### 受け入れ条件
1. Eventsourced trait は `receive_recover` メソッドを定義しなければならない
2. Eventsourced trait は `receive_snapshot` メソッドを定義しなければならない
3. Eventsourced trait は `receive_command` メソッドを定義しなければならない
4. Eventsourced trait は `on_recovery_completed` メソッドを定義しなければならない
5. Eventsourced trait は `on_persist_failure` / `on_persist_rejected` のデフォルト実装を提供しなければならない
6. Eventsourced trait は `on_recovery_failure` / `on_snapshot_failure` のデフォルト実装を提供しなければならない
7. Eventsourced trait は `journal_actor_ref` / `snapshot_actor_ref` / `last_sequence_nr` を提供しなければならない
8. Eventsourced trait を実装した型は状態マシンによる適切な状態管理を受けなければならない
9. Eventsourced 実装は Journal/SnapshotStore trait を直接呼び出してはならず、JournalActor/SnapshotActor へのメッセージ送信のみで実現しなければならない
10. Eventsourced 実装は Journal/SnapshotStore の Future を直接 poll してはならず、永続化結果は JournalResponse/SnapshotResponse を受信して処理しなければならない

### 要件16: PersistentActorBase の設計と実装

**目的:** 開発者として Eventsourced の実装に必要な共通インフラ（状態管理、メッセージルーティング、ハンドラーキューイング）を提供し、boilerplate コードを削減したい。

#### 受け入れ条件
1. PersistentActorBase は `PersistentActorState` を内部で管理しなければならない
2. PersistentActorBase は `pendingInvocations` キューを管理しなければならない
3. PersistentActorBase は `eventBatch` を管理し、複数イベントのバッチ化を行わなければならない
4. PersistentActorBase は `journalBatch` を管理し、JournalMessage::WriteMessages 用のバッチを構築しなければならない
5. PersistentActorBase は `flush_batch` メソッドを提供し、バッチを JournalActor に送信しなければならない
6. PersistentActorBase は JournalActor および SnapshotActor への ActorRef を保持しなければならない
7. PersistentActorBase は JournalResponse/SnapshotResponse を適切にディスパッチしなければならない
8. PersistentActorBase は `start_recovery` を提供し、PersistentActor の起動フック（pre_start 等）から明示的に呼び出せるようにしなければならない
9. PersistentActorBase は Journal/SnapshotStore の具体実装を保持してはならない
10. PersistentActorBase は Journal/SnapshotStore の Future を直接 poll してはならない

### 要件17: PersistentEnvelope の設計と実装

**目的:** 開発者として永続化対象のイベントと関連メタデータをカプセル化し、型安全な永続化パイプラインを実現したい。

#### 受け入れ条件
1. `PersistentEnvelope::new` が呼び出されたとき、PersistentEnvelope はイベントとハンドラーをカプセル化しなければならない
2. PersistentEnvelope は `sequence_nr` を保持しなければならない
3. PersistentEnvelope は `handler` 関数ポインタまたはクロージャを保持しなければならない
4. PersistentEnvelope は `stashing` フラグ（StashingHandlerInvocation か AsyncHandlerInvocation か）を保持しなければならない
5. PersistentEnvelope から PersistentRepr を生成できなければならない

### 要件18: PersistentActor trait の設計と実装

**目的:** 開発者として Pekko 互換の永続化アクターを実装し、メッセージパッシングによるイベントソーシングパターンを適用したい。

#### 受け入れ条件
1. PersistentActor は `persistence_id` メソッドを提供しなければならない
2. PersistentActor は `journal_actor_ref` メソッドを提供し、JournalActor への ActorRef を返さなければならない
3. PersistentActor は `snapshot_actor_ref` メソッドを提供し、SnapshotActor への ActorRef を返さなければならない
4. PersistentActor は `recovery` メソッドを提供し、Recovery 設定を返さなければならない
5. PersistentActor の実装は起動時（pre_start 等）に `start_recovery` を呼び出し、リカバリを開始しなければならない
6. Recovery が有効な間、PersistentActor は JournalResponse::ReplayedMessage を受信し `receive_recover` を呼び出さなければならない
7. スナップショットが存在する場合、PersistentActor は SnapshotResponse::LoadSnapshotResult を受信し `receive_snapshot` を呼び出さなければならない
8. Recovery が完了したとき、PersistentActor は `on_recovery_completed` を呼び出し ProcessingCommands 状態に遷移しなければならない
9. `persist` が呼び出されたとき、PersistentActor はイベントを eventBatch に追加しなければならない
10. バッチがフラッシュされたとき、PersistentActor は JournalActor に WriteMessages メッセージを送信しなければならない
11. `save_snapshot` が呼び出されたとき、PersistentActor は SnapshotActor に SaveSnapshot メッセージを送信しなければならない
12. `delete_messages` が呼び出されたとき、PersistentActor は JournalActor に DeleteMessagesTo メッセージを送信しなければならない
13. `delete_snapshots` が呼び出されたとき、PersistentActor は SnapshotActor に DeleteSnapshots メッセージを送信しなければならない
14. `Recovery::none()` が設定された場合、PersistentActor はイベント再生をスキップし highest_sequence_nr のみを取得しなければならない
15. PersistentActor は `base` / `base_mut` を提供し、PersistentActorBase への委譲を行えるようにしなければならない
16. PersistentActor は `start_recovery` / `handle_journal_response` / `handle_snapshot_response` を提供しなければならない

### 要件19: AtLeastOnceDelivery の設計と実装

**目的:** 開発者として信頼性のあるメッセージ配信を実装し、少なくとも1回の配信保証を提供したい。

#### 受け入れ条件
1. `deliver` が呼び出されたとき、AtLeastOnceDelivery はメッセージを送信し delivery_id を返さなければならない
2. `confirm_delivery` が呼び出されたとき、AtLeastOnceDelivery は該当する配信を未確認リストから削除しなければならない
3. 再配信間隔が経過したとき、AtLeastOnceDelivery は未確認メッセージを再送信しなければならない
4. `max_unconfirmed` を超えるメッセージが配信された場合、AtLeastOnceDelivery はエラーを返さなければならない
5. `get_delivery_snapshot` が呼び出されたとき、AtLeastOnceDelivery は現在の配信状態をスナップショットとして返さなければならない
6. `set_delivery_snapshot` が呼び出されたとき、AtLeastOnceDelivery は配信状態を復元しなければならない
7. `number_of_unconfirmed` が呼び出されたとき、AtLeastOnceDelivery は未確認配信の数を返さなければならない

### 要件20: PersistenceExtension の設計と実装

**目的:** 開発者として ActorSystem に永続化機能を拡張として追加し、JournalActor と SnapshotActor を一元的に管理したい。

#### 受け入れ条件
1. `PersistenceExtension::new` が呼び出されたとき、PersistenceExtension は JournalActor と SnapshotActor を作成しなければならない
2. PersistenceExtension の `journal_actor` メソッドが呼び出されたとき、JournalActor への ActorRef を返さなければならない
3. PersistenceExtension の `snapshot_actor` メソッドが呼び出されたとき、SnapshotActor への ActorRef を返さなければならない
4. PersistenceExtension は常に `Extension` trait を実装しなければならない
5. PersistenceExtension は Journal と SnapshotStore の具象型を受け取り、対応する Actor を生成しなければならない

### 要件21: InMemoryJournal の実装

**目的:** 開発者としてテスト環境で使用できる Journal 実装を提供し、永続化ロジックのテストを容易にしたい。

#### 受け入れ条件
1. `InMemoryJournal::new` が呼び出されたとき、InMemoryJournal は空のジャーナルを作成しなければならない
2. InMemoryJournal は常に `Journal` trait を実装しなければならない
3. InMemoryJournal はメッセージをメモリ上に保存しなければならない
4. InMemoryJournal は persistence_id ごとに独立したメッセージストレージを持たなければならない
5. InMemoryJournal のすべてのFuture関連型は `core::future::Ready<Result<T, E>>` でなければならない
6. InMemoryJournal は常に no_std 環境で動作しなければならない

### 要件22: InMemorySnapshotStore の実装

**目的:** 開発者としてテスト環境で使用できる SnapshotStore 実装を提供し、スナップショットロジックのテストを容易にしたい。

#### 受け入れ条件
1. `InMemorySnapshotStore::new` が呼び出されたとき、InMemorySnapshotStore は空のスナップショットストアを作成しなければならない
2. InMemorySnapshotStore は常に `SnapshotStore` trait を実装しなければならない
3. InMemorySnapshotStore はスナップショットをメモリ上に保存しなければならない
4. InMemorySnapshotStore は persistence_id ごとに独立したスナップショットストレージを持たなければならない
5. InMemorySnapshotStore のすべてのFuture関連型は `core::future::Ready<Result<T, E>>` でなければならない
6. InMemorySnapshotStore は常に no_std 環境で動作しなければならない

### 要件23: 単体テスト

**目的:** 開発者として各コンポーネントの実装が正しく動作することを検証し、回帰バグを防止したい。

#### 受け入れ条件
1. Journal trait のすべてのメソッドに対して単体テストが存在しなければならない
2. SnapshotStore trait のすべてのメソッドに対して単体テストが存在しなければならない
3. JournalMessage/JournalResponse のすべてのバリアントに対して単体テストが存在しなければならない
4. SnapshotMessage/SnapshotResponse のすべてのバリアントに対して単体テストが存在しなければならない
5. JournalActor のメッセージ処理に対して単体テストが存在しなければならない
6. SnapshotActor のメッセージ処理に対して単体テストが存在しなければならない
7. PersistentActorState の状態遷移に対して単体テストが存在しなければならない
8. PersistentRepr のすべてのメソッドに対して単体テストが存在しなければならない
9. SnapshotMetadata のすべてのメソッドに対して単体テストが存在しなければならない
10. SnapshotSelectionCriteria のすべてのメソッドに対して単体テストが存在しなければならない
11. Recovery のすべてのメソッドに対して単体テストが存在しなければならない
12. AtLeastOnceDelivery のすべてのメソッドに対して単体テストが存在しなければならない
13. PersistenceExtension のすべてのメソッドに対して単体テストが存在しなければならない
14. すべてのテストは `hoge/tests.rs` パターンに従って配置されなければならない

### 要件24: 使用例の提供

**目的:** 開発者として PersistentActor の使用方法を理解し、自身のアプリケーションに適用できるようにしたい。

#### 受け入れ条件
1. examples ディレクトリに基本的な PersistentActor の実装例が含まれなければならない
2. 使用例はメッセージパッシングによるイベントの永続化と再生を示さなければならない
3. 使用例はスナップショットの保存と復元を示さなければならない
4. 使用例は AtLeastOnceDelivery の使用方法を示さなければならない
5. 使用例は GATsパターンによるtrait実装と `core::future::ready()` の使用方法を示さなければならない
6. 使用例は no_std 環境での動作を前提としなければならない
7. 使用例は InMemoryJournal、InMemorySnapshotStore、JournalActor、SnapshotActor を使用しなければならない
8. 使用例は PersistentActorBase を使用した実装パターンを示さなければならない

### 要件25: Pekko 互換性

**目的:** 開発者として fraktor-persistence-rs が Pekko の永続化セマンティクスと互換性があることを確認したい。

#### 受け入れ条件
1. Journal の `write_messages` は Pekko の AsyncWriteJournal と同等のシーケンス番号検証を行わなければならない
2. SnapshotStore の `load_snapshot` は Pekko の SnapshotStore と同等の選択ロジックを実装しなければならない
3. Recovery の動作は Pekko の Recovery クラスと同等のセマンティクスを持たなければならない
4. AtLeastOnceDelivery は Pekko の AtLeastOnceDelivery と同等の再配信セマンティクスを持たなければならない
5. PersistentRepr は Pekko の PersistentRepr と同等のフィールドを持たなければならない
6. JournalMessage/JournalResponse は Pekko の journal プロトコルと同等のメッセージ種別を持たなければならない
7. PersistentActorState は Pekko の Eventsourced の状態遷移と同等のライフサイクルを持たなければならない
8. PendingHandlerInvocation は Pekko の pendingInvocations と同等のキューイングセマンティクスを持たなければならない

### 要件26: no_std 環境での動作保証

**目的:** 組み込みシステム開発者として fraktor-persistence-rs が no_std 環境で正しく動作することを確認したい。

#### 受け入れ条件
1. すべてのコードは `#![no_std]` 環境でコンパイル可能でなければならない
2. すべてのコードは `alloc` クレートのみに依存し、`std` クレートに依存してはならない
3. `cfg-std-forbid` lint に違反するコードが core モジュールに含まれてはならない
4. `modules/persistence/src` 内で `#[cfg(feature = "std")]` による機能分岐を使用してはならない
5. `core::future::Ready<T>` はプロジェクト全体で統一的に使用されなければならない

### 要件27: エラーハンドリング

**目的:** 開発者として永続化操作のエラーを適切に処理し、信頼性のあるシステムを構築したい。

#### 受け入れ条件
1. `JournalError` 列挙型が定義され、ジャーナル操作のすべてのエラーケースをカバーしなければならない
2. `SnapshotError` 列挙型が定義され、スナップショット操作のすべてのエラーケースをカバーしなければならない
3. `PersistenceError` 列挙型が定義され、永続化全般のエラーを統合しなければならない
4. すべてのエラー型は `Debug` と `Display` を実装しなければならない
5. すべてのエラー型は no_std 環境で動作しなければならない

### 要件28: モジュール構造

**目的:** 開発者として fraktor-rs のモジュール構造規約に従い、一貫性のあるコードベースを維持したい。

#### 受け入れ条件
1. すべてのファイルは 2018 モジュール構成（`foo.rs` + `foo/` ディレクトリ）に従わなければならない
2. `mod.rs` を使用してはならない
3. 1 ファイルに複数のパブリック構造体、trait、enum を含めてはならない
4. 単体テストは `hoge.rs` に対して `hoge/tests.rs` に配置しなければならない
5. 公開 API は `core` モジュール直下で再エクスポートしなければならない
6. 内部参照は FQCN (`crate::...`) で明示的に記述しなければならない
