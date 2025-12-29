# 要件ドキュメント

## 導入

本ドキュメントは、fraktor-persistence-rs クレートの Untyped 永続化モジュールに対する要件を定義する。既存の `modules/persistence/src` 以下のソースファイルは全て削除し、完全に再設計・再実装を行う。

本モジュールは以下の設計方針に従う:

1. **GATsパターン + Future型**: `async fn` を使用せず、trait内でGeneric Associated Types（GATs）を使用してFuture型を関連型として定義する。同期実装では `core::future::Ready<T>` を返し、非同期実装では独自Future型を返す。

2. **`core::future::Ready<T>`の導入と横展開**: 同期実装（InMemory等）では `core::future::ready()` を使用してFutureを返す。他モジュール（cluster等）への横展開も今回のスコープに含める。

3. **no_std専用**: `#![no_std]` 環境のみで実装し、`alloc` クレートに依存する。coreモジュールではstdに一切依存しない。std 向け拡張は今回のスコープ外とする。

4. **Pekko仕様互換**: Apache Pekko の永続化セマンティクスと互換性を持ち、イベントソーシングと CQRS パターンをサポートする。

主要コンポーネント:
- `Journal` trait（GATsパターンによるイベントジャーナル抽象化）
- `SnapshotStore` trait（GATsパターンによるスナップショットストア抽象化）
- `PersistentActor` trait（永続化アクター抽象化）
- `PersistentRepr`（永続化イベント表現）
- `Recovery`（リカバリ設定）
- `SnapshotMetadata` / `SnapshotSelectionCriteria`（スナップショット関連）
- `AtLeastOnceDelivery`（少なくとも1回配信保証）
- `PersistenceExtension`（永続化拡張）
- `InMemoryJournal` / `InMemorySnapshotStore`（テスト用実装、`core::future::ready()`使用）

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

### 要件9: PersistentActor trait の設計と実装

**目的:** 開発者として Pekko 互換の永続化アクターを実装し、イベントソーシングパターンを適用したい。

#### 受け入れ条件
1. PersistentActor が起動したとき、永続化モジュールは `perform_recovery` を実行しなければならない
2. Recovery が有効な間、PersistentActor は保存されたイベントを `receive_recover` で再生しなければならない
3. スナップショットが存在する場合、PersistentActor は `receive_snapshot` を呼び出してから残りのイベントを再生しなければならない
4. Recovery が完了したとき、PersistentActor は `on_recovery_completed` を呼び出さなければならない
5. `persist` が呼び出されたとき、PersistentActor はイベントを Journal に書き込み sequence_nr を更新しなければならない
6. `persist_all` が呼び出されたとき、PersistentActor は複数イベントを連続したシーケンス番号で書き込まなければならない
7. `save_snapshot` が呼び出されたとき、PersistentActor はスナップショットを SnapshotStore に保存しなければならない
8. `delete_messages` が呼び出されたとき、PersistentActor は指定されたシーケンス番号以下のイベントを削除しなければならない
9. `delete_snapshots` が呼び出されたとき、PersistentActor は条件に一致するスナップショットを削除しなければならない
10. `Recovery::none()` が設定された場合、PersistentActor はイベント再生をスキップし highest_sequence_nr のみを取得しなければならない

### 要件10: AtLeastOnceDelivery の設計と実装

**目的:** 開発者として信頼性のあるメッセージ配信を実装し、少なくとも1回の配信保証を提供したい。

#### 受け入れ条件
1. `deliver` が呼び出されたとき、AtLeastOnceDelivery はメッセージを送信し delivery_id を返さなければならない
2. `confirm_delivery` が呼び出されたとき、AtLeastOnceDelivery は該当する配信を未確認リストから削除しなければならない
3. 再配信間隔が経過したとき、AtLeastOnceDelivery は未確認メッセージを再送信しなければならない
4. `max_unconfirmed` を超えるメッセージが配信された場合、AtLeastOnceDelivery はエラーを返さなければならない
5. `get_delivery_snapshot` が呼び出されたとき、AtLeastOnceDelivery は現在の配信状態をスナップショットとして返さなければならない
6. `set_delivery_snapshot` が呼び出されたとき、AtLeastOnceDelivery は配信状態を復元しなければならない
7. `number_of_unconfirmed` が呼び出されたとき、AtLeastOnceDelivery は未確認配信の数を返さなければならない

### 要件11: PersistenceExtension の設計と実装

**目的:** 開発者として ActorSystem に永続化機能を拡張として追加し、一元的に管理したい。

#### 受け入れ条件
1. `PersistenceExtension::new` が呼び出されたとき、PersistenceExtension は Journal と SnapshotStore を保持しなければならない
2. PersistenceExtension の `journal` メソッドが呼び出されたとき、内部の Journal への参照を返さなければならない
3. PersistenceExtension の `snapshot_store` メソッドが呼び出されたとき、内部の SnapshotStore への参照を返さなければならない
4. PersistenceExtension は常に `Extension` trait を実装しなければならない

### 要件12: InMemoryJournal の実装

**目的:** 開発者としてテスト環境で使用できる Journal 実装を提供し、永続化ロジックのテストを容易にしたい。

#### 受け入れ条件
1. `InMemoryJournal::new` が呼び出されたとき、InMemoryJournal は空のジャーナルを作成しなければならない
2. InMemoryJournal は常に `Journal` trait を実装しなければならない
3. InMemoryJournal はメッセージをメモリ上に保存しなければならない
4. InMemoryJournal は persistence_id ごとに独立したメッセージストレージを持たなければならない
5. InMemoryJournal のすべてのFuture関連型は `core::future::Ready<Result<T, E>>` でなければならない
6. InMemoryJournal は常に no_std 環境で動作しなければならない

### 要件13: InMemorySnapshotStore の実装

**目的:** 開発者としてテスト環境で使用できる SnapshotStore 実装を提供し、スナップショットロジックのテストを容易にしたい。

#### 受け入れ条件
1. `InMemorySnapshotStore::new` が呼び出されたとき、InMemorySnapshotStore は空のスナップショットストアを作成しなければならない
2. InMemorySnapshotStore は常に `SnapshotStore` trait を実装しなければならない
3. InMemorySnapshotStore はスナップショットをメモリ上に保存しなければならない
4. InMemorySnapshotStore は persistence_id ごとに独立したスナップショットストレージを持たなければならない
5. InMemorySnapshotStore のすべてのFuture関連型は `core::future::Ready<Result<T, E>>` でなければならない
6. InMemorySnapshotStore は常に no_std 環境で動作しなければならない

### 要件14: 単体テスト

**目的:** 開発者として各コンポーネントの実装が正しく動作することを検証し、回帰バグを防止したい。

#### 受け入れ条件
1. Journal trait のすべてのメソッドに対して単体テストが存在しなければならない
2. SnapshotStore trait のすべてのメソッドに対して単体テストが存在しなければならない
3. PersistentRepr のすべてのメソッドに対して単体テストが存在しなければならない
4. SnapshotMetadata のすべてのメソッドに対して単体テストが存在しなければならない
5. SnapshotSelectionCriteria のすべてのメソッドに対して単体テストが存在しなければならない
6. Recovery のすべてのメソッドに対して単体テストが存在しなければならない
7. AtLeastOnceDelivery のすべてのメソッドに対して単体テストが存在しなければならない
8. PersistenceExtension のすべてのメソッドに対して単体テストが存在しなければならない
9. すべてのテストは `hoge/tests.rs` パターンに従って配置されなければならない
10. ~~clusterモジュールの横展開対象に対しても単体テストが存在しなければならない~~（スコープ外: 要件3 参照）

### 要件15: 使用例の提供

**目的:** 開発者として PersistentActor の使用方法を理解し、自身のアプリケーションに適用できるようにしたい。

#### 受け入れ条件
1. examples ディレクトリに基本的な PersistentActor の実装例が含まれなければならない
2. 使用例はイベントの永続化と再生を示さなければならない
3. 使用例はスナップショットの保存と復元を示さなければならない
4. 使用例は AtLeastOnceDelivery の使用方法を示さなければならない
5. 使用例は GATsパターンによるtrait実装と `core::future::ready()` の使用方法を示さなければならない
6. 使用例は no_std 環境での動作を前提としなければならない
7. 使用例は InMemoryJournal と InMemorySnapshotStore を使用しなければならない

### 要件16: Pekko 互換性

**目的:** 開発者として fraktor-persistence-rs が Pekko の永続化セマンティクスと互換性があることを確認したい。

#### 受け入れ条件
1. Journal の `write_messages` は Pekko の AsyncWriteJournal と同等のシーケンス番号検証を行わなければならない
2. SnapshotStore の `load_snapshot` は Pekko の SnapshotStore と同等の選択ロジックを実装しなければならない
3. Recovery の動作は Pekko の Recovery クラスと同等のセマンティクスを持たなければならない
4. AtLeastOnceDelivery は Pekko の AtLeastOnceDelivery と同等の再配信セマンティクスを持たなければならない
5. PersistentRepr は Pekko の PersistentRepr と同等のフィールドを持たなければならない

### 要件17: no_std 環境での動作保証

**目的:** 組み込みシステム開発者として fraktor-persistence-rs が no_std 環境で正しく動作することを確認したい。

#### 受け入れ条件
1. すべてのコードは `#![no_std]` 環境でコンパイル可能でなければならない
2. すべてのコードは `alloc` クレートのみに依存し、`std` クレートに依存してはならない
3. `cfg-std-forbid` lint に違反するコードが core モジュールに含まれてはならない
4. `modules/persistence/src` 内で `#[cfg(feature = "std")]` による機能分岐を使用してはならない
5. `core::future::Ready<T>` はプロジェクト全体で統一的に使用されなければならない

### 要件18: エラーハンドリング

**目的:** 開発者として永続化操作のエラーを適切に処理し、信頼性のあるシステムを構築したい。

#### 受け入れ条件
1. `JournalError` 列挙型が定義され、ジャーナル操作のすべてのエラーケースをカバーしなければならない
2. `SnapshotError` 列挙型が定義され、スナップショット操作のすべてのエラーケースをカバーしなければならない
3. `PersistenceError` 列挙型が定義され、永続化全般のエラーを統合しなければならない
4. すべてのエラー型は `Debug` と `Display` を実装しなければならない
5. すべてのエラー型は no_std 環境で動作しなければならない

### 要件19: モジュール構造

**目的:** 開発者として fraktor-rs のモジュール構造規約に従い、一貫性のあるコードベースを維持したい。

#### 受け入れ条件
1. すべてのファイルは 2018 モジュール構成（`foo.rs` + `foo/` ディレクトリ）に従わなければならない
2. `mod.rs` を使用してはならない
3. 1 ファイルに複数のパブリック構造体、trait、enum を含めてはならない
4. 単体テストは `hoge.rs` に対して `hoge/tests.rs` に配置しなければならない
5. 公開 API は `prelude` モジュールで提供しなければならない
6. 内部参照は FQCN (`crate::...`) で明示的に記述しなければならない
