# 実装計画

## タスク一覧

- [ ] 1. 基盤エラー型の実装
- [ ] 1.1 (P) Journal 操作エラー型の実装
  - シーケンス番号不整合、書き込み失敗、読み込み失敗、削除失敗のバリアントを持つ列挙型を作成する
  - Debug と Display trait を実装する
  - no_std 環境で動作することを確認する
  - _Requirements: 18.1, 18.2, 18.4, 18.5_

- [ ] 1.2 (P) Snapshot 操作エラー型の実装
  - 保存失敗、読み込み失敗、削除失敗のバリアントを持つ列挙型を作成する
  - Debug と Display trait を実装する
  - no_std 環境で動作することを確認する
  - _Requirements: 18.2, 18.4, 18.5_

- [ ] 1.3 永続化統合エラー型の実装
  - Journal エラーと Snapshot エラーを統合する列挙型を作成する
  - リカバリエラーとリカバリ中不正操作のバリアントを追加する
  - From trait による変換を実装する
  - Debug と Display trait を実装する
  - _Requirements: 18.3, 18.4, 18.5_

- [ ] 2. スナップショット関連データ型の実装
- [ ] 2.1 (P) スナップショットメタデータ型の実装
  - persistence_id, sequence_nr, timestamp を保持する構造体を作成する
  - Clone, Debug, PartialEq, Eq, PartialOrd, Ord を derive する
  - 各フィールドへのアクセサを提供する
  - 単体テストを作成する
  - _Requirements: 7.1, 7.2, 7.3, 7.4_

- [ ] 2.2 (P) スナップショット選択条件型の実装
  - max/min の sequence_nr と timestamp を条件として保持する構造体を作成する
  - latest() で最新スナップショット選択条件を返す
  - none() ですべてにマッチしない条件を返す
  - matches() でメタデータが条件にマッチするか判定する
  - limit() で max_sequence_nr を制限した新条件を返す
  - 単体テストを作成する
  - _Requirements: 7.5, 7.6, 7.7, 7.8_

- [ ] 2.3 スナップショット型の実装
  - メタデータとデータ（ArcShared<dyn Any + Send + Sync>）を保持する構造体を作成する
  - メタデータへのアクセサとデータのダウンキャストメソッドを提供する
  - _Requirements: 5.3, 5.4, 5.5_

- [ ] 3. 永続化イベント表現型の実装
- [ ] 3.1 永続化イベント表現型の実装
  - persistence_id, sequence_nr, payload, manifest, writer_uuid, timestamp, metadata を保持する構造体を作成する
  - Clone を derive する
  - 各フィールドへのアクセサを提供する
  - ダウンキャスト用メソッドを実装する
  - with_manifest, with_metadata, with_timestamp, with_writer_uuid ビルダーメソッドを実装する
  - 単体テストを作成する
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6_

- [ ] 4. リカバリ設定型の実装
- [ ] 4.1 リカバリ設定型の実装
  - スナップショット選択条件、to_sequence_nr、replay_max を保持する構造体を作成する
  - Clone を derive する
  - default() で最新スナップショットから全イベント再生する設定を返す
  - none() でリカバリをスキップする設定を返す
  - from_snapshot() で指定条件でスナップショットを選択する設定を返す
  - 単体テストを作成する
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6_

- [ ] 5. Journal trait と実装
- [ ] 5.1 Journal trait の定義
  - GATsパターンで WriteFuture, ReplayFuture, DeleteFuture, HighestSeqNrFuture を関連型として定義する
  - write_messages メソッドを定義する（複数イベントを一括書き込み）
  - replay_messages メソッドを定義する（指定範囲のイベントを再生）
  - delete_messages_to メソッドを定義する（指定シーケンス番号以下を削除）
  - highest_sequence_nr メソッドを定義する（最大シーケンス番号を取得）
  - すべての Future 型は Send + 'a を満たすことを確認する
  - _Requirements: 1.1, 1.3, 1.4, 1.5, 1.8, 1.9, 4.1, 4.4, 4.7, 4.9_

- [ ] 5.2 InMemory Journal 実装
  - BTreeMap を使用して persistence_id ごとにイベントを保存する
  - highest_sequence_nr を別途管理する
  - すべての Future 型に core::future::Ready<Result<T, E>> を使用する
  - write_messages でシーケンス番号の連続性を検証する
  - replay_messages で max パラメータによる件数制限を実装する
  - delete_messages_to で指定シーケンス番号以下を削除する
  - 単体テストを作成する
  - _Requirements: 2.1, 2.3, 2.4, 2.5, 2.6, 4.2, 4.3, 4.5, 4.6, 4.8, 4.10, 4.11, 12.1, 12.2, 12.3, 12.4, 12.5, 12.6_

- [ ] 5.3 Journal 共有ラッパーの実装
  - ArcShared<ToolboxMutex<J>> パターンで Journal を共有可能にする
  - SharedAccess<J> trait を実装する
  - Clone を実装する
  - プロジェクトの共有ラッパー設計ガイドに準拠する
  - _Requirements: 1.1, 4.1, 11.2_

- [ ] 6. SnapshotStore trait と実装
- [ ] 6.1 SnapshotStore trait の定義
  - GATsパターンで SaveFuture, LoadFuture, DeleteOneFuture, DeleteManyFuture を関連型として定義する
  - save_snapshot メソッドを定義する（スナップショットを保存）
  - load_snapshot メソッドを定義する（条件に一致する最新スナップショットを取得）
  - delete_snapshot メソッドを定義する（指定メタデータのスナップショットを削除）
  - delete_snapshots メソッドを定義する（条件に一致するすべてを削除）
  - すべての Future 型は Send + 'a を満たすことを確認する
  - _Requirements: 1.2, 1.3, 1.6, 1.7, 1.8, 1.9, 5.1, 5.3, 5.6, 5.8_

- [ ] 6.2 InMemory SnapshotStore 実装
  - BTreeMap を使用して persistence_id ごとにスナップショットを保存する
  - すべての Future 型に core::future::Ready<Result<T, E>> を使用する
  - save_snapshot でメタデータとデータを保存する
  - load_snapshot で条件に一致する最新スナップショットを返す
  - delete_snapshot で指定メタデータのスナップショットを削除する
  - delete_snapshots で条件に一致するすべてを削除する
  - 単体テストを作成する
  - _Requirements: 2.2, 2.3, 2.4, 2.5, 2.6, 5.2, 5.4, 5.5, 5.7, 5.9, 13.1, 13.2, 13.3, 13.4, 13.5, 13.6_

- [ ] 6.3 SnapshotStore 共有ラッパーの実装
  - ArcShared<ToolboxMutex<S>> パターンで SnapshotStore を共有可能にする
  - SharedAccess<S> trait を実装する
  - Clone を実装する
  - プロジェクトの共有ラッパー設計ガイドに準拠する
  - _Requirements: 1.2, 5.1, 11.3_

- [ ] 7. PersistentActor trait の実装
- [ ] 7.1 PersistentActor trait の定義
  - Journal と SnapshotStore を関連型として定義する
  - persistence_id, journal, journal_mut, snapshot_store, snapshot_store_mut メソッドを定義する
  - recovery メソッドをデフォルト実装で提供する
  - receive_recover, receive_snapshot, on_recovery_completed コールバックを定義する
  - last_sequence_nr メソッドを定義する
  - persist, persist_all メソッドを定義する（イベント永続化）
  - save_snapshot, delete_messages, delete_snapshots メソッドを定義する
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6, 9.7, 9.8, 9.9, 9.10_

- [ ] 7.2 poll_ready ヘルパーの実装
  - no_std 環境で core::future::Ready<T> を即座に完了させるヘルパー関数を作成する
  - no-op waker を使用して poll を 1 回で完了させる
  - 非 Ready Future の場合はパニックする
  - _Requirements: 2.4, 2.5, 9.5, 9.6, 9.7_

- [ ] 8. AtLeastOnceDelivery の実装
- [ ] 8.1 (P) 配信設定型の実装
  - 再配信間隔、最大未確認数、再配信バースト制限を保持する構造体を作成する
  - デフォルト値を提供する
  - _Requirements: 10.3, 10.4_

- [ ] 8.2 (P) 未確認配信型の実装
  - delivery_id, destination, payload, sender, timestamp を保持する構造体を作成する
  - Toolbox ジェネリックで ActorRef と TimerInstant を扱う
  - _Requirements: 10.1, 10.5_

- [ ] 8.3 (P) 配信状態スナップショット型の実装
  - 現在の delivery_id と未確認配信リストを保持する構造体を作成する
  - リカバリ時の状態復元に使用する
  - _Requirements: 10.5, 10.6_

- [ ] 8.4 AtLeastOnceDelivery ジェネリック構造体の実装
  - 設定、次の delivery_id、未確認配信リスト、再配信ハンドルを保持する
  - deliver メソッドで配信し delivery_id を返す
  - confirm_delivery メソッドで配信確認し再配信をキャンセルする
  - handle_message メソッドで内部 RedeliveryTick を処理する
  - get_delivery_snapshot で現在の配信状態をスナップショットとして取得する
  - set_delivery_snapshot でスナップショットから状態を復元する
  - number_of_unconfirmed で未確認配信数を返す
  - max_unconfirmed を超える配信でエラーを返す
  - Scheduler 経由で再配信間隔後に再送信する
  - 単体テストを作成する
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6, 10.7_

- [ ] 9. PersistenceExtension の実装
- [ ] 9.1 PersistenceExtension の実装
  - JournalShared と SnapshotStoreShared を保持する構造体を作成する
  - new メソッドで Journal と SnapshotStore を受け取り共有ラッパーを作成する
  - journal メソッドで JournalShared への参照を返す
  - snapshot_store メソッドで SnapshotStoreShared への参照を返す
  - Clone を実装する
  - Extension trait を実装する
  - 単体テストを作成する
  - _Requirements: 11.1, 11.2, 11.3, 11.4_

- [ ] 10. モジュール構造と Prelude の整備
- [ ] 10.1 モジュール構造の整備
  - lib.rs でクレートルートを設定する
  - core.rs で core モジュールエントリを作成する
  - 各コンポーネントを適切なファイルに配置する
  - 2018 モジュール構成に従う（foo.rs + foo/ パターン）
  - mod.rs を使用しない
  - FQCN インポートを使用する
  - _Requirements: 19.1, 19.2, 19.3, 19.4, 19.6_

- [ ] 10.2 Prelude モジュールの実装
  - 公開 API を prelude に集約する
  - 主要な trait と型をエクスポートする
  - ユーザーが import しやすい構造を提供する
  - _Requirements: 19.5_

- [ ] 11. no_std ビルド検証
- [ ] 11.1 no_std ビルド検証
  - #![no_std] でコンパイルできることを確認する
  - alloc クレートのみに依存していることを確認する
  - std クレートへの依存がないことを確認する
  - cfg-std-forbid lint に違反しないことを確認する
  - THUMB ターゲットでのビルドを確認する
  - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5_

- [ ] 12. Pekko 互換性検証
- [ ] 12.1 Pekko 互換性検証
  - Journal の write_messages が Pekko の AsyncWriteJournal と同等のシーケンス番号検証を行うことを確認する
  - SnapshotStore の load_snapshot が Pekko の SnapshotStore と同等の選択ロジックを持つことを確認する
  - Recovery の動作が Pekko の Recovery と同等のセマンティクスを持つことを確認する
  - AtLeastOnceDelivery が Pekko と同等の再配信セマンティクスを持つことを確認する
  - PersistentRepr が Pekko と同等のフィールドを持つことを確認する
  - _Requirements: 16.1, 16.2, 16.3, 16.4, 16.5_

- [ ] 13. 使用例の作成
- [ ] 13.1 基本的な PersistentActor 使用例の作成
  - InMemoryJournal と InMemorySnapshotStore を使用した例を作成する
  - イベントの永続化と再生を示す
  - スナップショットの保存と復元を示す
  - GATsパターンと core::future::ready() の使用方法を示す
  - no_std 環境での動作を前提とする
  - _Requirements: 15.1, 15.2, 15.3, 15.5, 15.6, 15.7_

- [ ] 13.2 AtLeastOnceDelivery 使用例の作成
  - deliver と confirm_delivery の使用方法を示す
  - handle_message によるRedeliveryTick 処理を示す
  - 配信状態のスナップショットと復元を示す
  - _Requirements: 15.4_

- [ ] 14. 統合テストと最終検証
- [ ] 14.1 統合テストの作成
  - PersistentActor のリカバリフローをテストする
  - スナップショットからのリカバリをテストする
  - イベント再生をテストする
  - AtLeastOnceDelivery の配信と確認をテストする
  - PersistenceExtension による統合をテストする
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 10.1, 10.2, 11.1_

- [ ] 14.2 CI チェックの実行
  - scripts/ci-check.sh all を実行する
  - すべてのテストがパスすることを確認する
  - lint エラーがないことを確認する
  - clippy 警告がないことを確認する
  - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5, 19.1, 19.2, 19.3, 19.4, 19.5, 19.6_
