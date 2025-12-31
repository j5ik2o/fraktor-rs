# 実装計画

## タスク一覧

- [x] 1. 基盤エラー型の実装
- [x] 1.1 (P) Journal 操作エラー型の実装
  - シーケンス番号不整合、書き込み失敗、読み込み失敗、削除失敗のバリアントを持つ列挙型を作成する
  - Debug と Display trait を実装する
  - no_std 環境で動作することを確認する
  - _Requirements: 27.1, 27.4, 27.5_

- [x] 1.2 (P) Snapshot 操作エラー型の実装
  - 保存失敗、読み込み失敗、削除失敗のバリアントを持つ列挙型を作成する
  - Debug と Display trait を実装する
  - no_std 環境で動作することを確認する
  - _Requirements: 27.2, 27.4, 27.5_

- [x] 1.3 永続化統合エラー型の実装
  - Journal エラーと Snapshot エラーを統合する列挙型を作成する
  - リカバリエラー、状態マシンエラー、メッセージパッシングエラーのバリアントを追加する
  - From trait による変換を実装する
  - Debug と Display trait を実装する
  - _Requirements: 27.3, 27.4, 27.5_

- [x] 2. スナップショット関連データ型の実装
- [x] 2.1 (P) スナップショットメタデータ型の実装
  - persistence_id, sequence_nr, timestamp を保持する構造体を作成する
  - Clone, Debug, PartialEq, Eq, PartialOrd, Ord を derive する
  - 各フィールドへのアクセサを提供する
  - 単体テストを作成する
  - _Requirements: 7.1, 7.2, 7.3, 7.4_

- [x] 2.2 (P) スナップショット選択条件型の実装
  - max/min の sequence_nr と timestamp を条件として保持する構造体を作成する
  - latest() で最新スナップショット選択条件を返す
  - none() ですべてにマッチしない条件を返す
  - matches() でメタデータが条件にマッチするか判定する
  - limit() で max_sequence_nr を制限した新条件を返す
  - 単体テストを作成する
  - _Requirements: 7.5, 7.6, 7.7, 7.8_

- [x] 2.3 スナップショット型の実装
  - メタデータとデータ（ArcShared<dyn Any + Send + Sync>）を保持する構造体を作成する
  - メタデータへのアクセサとデータのダウンキャストメソッドを提供する
  - _Requirements: 5.3, 5.4, 5.5_

- [x] 3. 永続化イベント表現型の実装
- [x] 3.1 永続化イベント表現型の実装
  - persistence_id, sequence_nr, payload, manifest, writer_uuid, timestamp, metadata を保持する構造体を作成する
  - Clone を derive する
  - 各フィールドへのアクセサを提供する
  - ダウンキャスト用メソッドを実装する
  - with_manifest, with_metadata, with_timestamp, with_writer_uuid ビルダーメソッドを実装する
  - 単体テストを作成する
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6_

- [x] 4. リカバリ設定型の実装
- [x] 4.1 リカバリ設定型の実装
  - スナップショット選択条件、to_sequence_nr、replay_max を保持する構造体を作成する
  - Clone を derive する
  - default() で最新スナップショットから全イベント再生する設定を返す
  - none() でリカバリをスキップする設定を返す
  - from_snapshot() で指定条件でスナップショットを選択する設定を返す
  - 単体テストを作成する
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6_

- [x] 5. Journal trait と実装
- [x] 5.1 Journal trait の定義
  - GATsパターンで WriteFuture, ReplayFuture, DeleteFuture, HighestSeqNrFuture を関連型として定義する
  - write_messages メソッドを定義する（複数イベントを一括書き込み）
  - replay_messages メソッドを定義する（指定範囲のイベントを再生）
  - delete_messages_to メソッドを定義する（指定シーケンス番号以下を削除）
  - highest_sequence_nr メソッドを定義する（最大シーケンス番号を取得）
  - すべての Future 型は Send + 'a を満たすことを確認する
  - _Requirements: 1.1, 1.3, 1.4, 1.5, 1.8, 1.9, 4.1, 4.4, 4.7, 4.9_

- [x] 5.2 InMemory Journal 実装
  - BTreeMap を使用して persistence_id ごとにイベントを保存する
  - highest_sequence_nr を別途管理する
  - すべての Future 型に core::future::Ready<Result<T, E>> を使用する
  - write_messages でシーケンス番号の連続性を検証する
  - replay_messages で max パラメータによる件数制限を実装する
  - delete_messages_to で指定シーケンス番号以下を削除する
  - 単体テストを作成する
  - _Requirements: 2.1, 2.3, 2.4, 2.5, 2.6, 4.2, 4.3, 4.5, 4.6, 4.8, 4.10, 4.11, 21.1, 21.2, 21.3, 21.4, 21.5, 21.6_

- [x] 6. SnapshotStore trait と実装
- [x] 6.1 SnapshotStore trait の定義
  - GATsパターンで SaveFuture, LoadFuture, DeleteOneFuture, DeleteManyFuture を関連型として定義する
  - save_snapshot メソッドを定義する（スナップショットを保存）
  - load_snapshot メソッドを定義する（条件に一致する最新スナップショットを取得）
  - delete_snapshot メソッドを定義する（指定メタデータのスナップショットを削除）
  - delete_snapshots メソッドを定義する（条件に一致するすべてを削除）
  - すべての Future 型は Send + 'a を満たすことを確認する
  - _Requirements: 1.2, 1.3, 1.6, 1.7, 1.8, 1.9, 5.1, 5.3, 5.6, 5.8_

- [x] 6.2 InMemory SnapshotStore 実装
  - BTreeMap を使用して persistence_id ごとにスナップショットを保存する
  - すべての Future 型に core::future::Ready<Result<T, E>> を使用する
  - save_snapshot でメタデータとデータを保存する
  - load_snapshot で条件に一致する最新スナップショットを返す
  - delete_snapshot で指定メタデータのスナップショットを削除する
  - delete_snapshots で条件に一致するすべてを削除する
  - 単体テストを作成する
  - _Requirements: 2.2, 2.3, 2.4, 2.5, 2.6, 5.2, 5.4, 5.5, 5.7, 5.9, 22.1, 22.2, 22.3, 22.4, 22.5, 22.6_

- [x] 7. メッセージプロトコルの実装
- [x] 7.1 JournalMessage の実装
  - WriteMessages バリアント（persistence_id, to_sequence_nr, messages: Vec<PersistentRepr>, sender: ActorRef, instance_id）を定義する
  - ReplayMessages バリアント（persistence_id, from_seq, to_seq, max, sender: ActorRef）を定義する
  - DeleteMessagesTo バリアント（persistence_id, to_sequence_nr, sender: ActorRef）を定義する
  - GetHighestSequenceNr バリアント（persistence_id, from_sequence_nr, sender: ActorRef）を定義する
  - Clone, Debug を derive する
  - 単体テストを作成する
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.10_

- [x] 7.2 JournalResponse の実装
  - WriteMessageSuccess バリアント（repr, instance_id）を定義する
  - WriteMessageFailure バリアント（repr, error, instance_id）を定義する
  - WriteMessageRejected バリアント（repr, error, instance_id）を定義する
  - WriteMessagesSuccessful バリアントを定義する
  - WriteMessagesFailed バリアント（cause, write_count）を定義する
  - ReplayedMessage バリアント（persistent_repr）を定義する
  - RecoverySuccess バリアント（highest_sequence_nr）を定義する
  - HighestSequenceNr バリアント（persistence_id, sequence_nr）を定義する
  - ReplayMessagesFailure バリアント（JournalError）を定義する
  - DeleteMessagesSuccess バリアント（to_sequence_nr）を定義する
  - DeleteMessagesFailure バリアント（cause, to_sequence_nr）を定義する
  - Clone, Debug を derive する
  - 単体テストを作成する
  - _Requirements: 9.5, 9.6, 9.7, 9.8, 9.9_

- [x] 7.3 SnapshotMessage の実装
  - SaveSnapshot バリアント（metadata, snapshot, sender: ActorRef）を定義する
  - LoadSnapshot バリアント（persistence_id, criteria, sender: ActorRef）を定義する
  - DeleteSnapshot バリアント（metadata, sender: ActorRef）を定義する
  - DeleteSnapshots バリアント（persistence_id, criteria, sender: ActorRef）を定義する
  - Clone, Debug を derive する
  - 単体テストを作成する
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.8_

- [x] 7.4 SnapshotResponse の実装
  - SaveSnapshotSuccess バリアント（metadata）を定義する
  - SaveSnapshotFailure バリアント（metadata, error）を定義する
  - LoadSnapshotResult バリアント（Option<Snapshot>, to_sequence_nr）を定義する
  - LoadSnapshotFailed バリアント（SnapshotError）を定義する
  - DeleteSnapshotSuccess バリアント（metadata）を定義する
  - DeleteSnapshotsSuccess バリアント（criteria）を定義する
  - DeleteSnapshotFailure バリアント（metadata, error）を定義する
  - DeleteSnapshotsFailure バリアント（criteria, error）を定義する
  - Clone, Debug を derive する
  - 単体テストを作成する
  - _Requirements: 10.5, 10.6, 10.7_

- [x] 8. JournalActor の実装
- [x] 8.1 JournalActor の基本構造
  - Journal trait をジェネリックパラメータとして受け取る構造体を作成する
  - RuntimeToolbox パラメータに対応する
  - ActorReceiver trait を実装する
  - JournalMessage を receive_message で処理する
  - _Requirements: 11.1, 11.4, 11.5, 11.6_

- [x] 8.2 JournalActor のメッセージ処理
  - WriteMessages を受信したら journal.write_messages を呼び出し、結果を sender に返す
  - ReplayMessages を受信したら journal.replay_messages を呼び出し、各イベントを ReplayedMessage で返す
  - DeleteMessagesTo を受信したら journal.delete_messages_to を呼び出し、結果を sender に返す
  - GetHighestSequenceNr を受信したら journal.highest_sequence_nr を呼び出し、結果を sender に返す
  - `Poll::Pending` の場合は JournalActor 自身に再ポーリング依頼メッセージを送信する（Dispatcher を直接呼び出さない、drive_ready を使わない）
  - 単体テストを作成する
  - _Requirements: 11.2, 11.3, 11.8_

- [x] 9. SnapshotActor の実装
- [x] 9.1 SnapshotActor の基本構造
  - SnapshotStore trait をジェネリックパラメータとして受け取る構造体を作成する
  - RuntimeToolbox パラメータに対応する
  - ActorReceiver trait を実装する
  - SnapshotMessage を receive_message で処理する
  - _Requirements: 12.1, 12.4, 12.5, 12.6_

- [x] 9.2 SnapshotActor のメッセージ処理
  - SaveSnapshot を受信したら snapshot_store.save_snapshot を呼び出し、結果を sender に返す
  - LoadSnapshot を受信したら snapshot_store.load_snapshot を呼び出し、結果を sender に返す
  - DeleteSnapshot を受信したら snapshot_store.delete_snapshot を呼び出し、結果を sender に返す
  - DeleteSnapshots を受信したら snapshot_store.delete_snapshots を呼び出し、結果を sender に返す
  - `Poll::Pending` の場合は SnapshotActor 自身に再ポーリング依頼メッセージを送信する（Dispatcher を直接呼び出さない、drive_ready を使わない）
  - 単体テストを作成する
  - _Requirements: 12.2, 12.3, 12.8_

- [x] 10. 状態管理の実装
- [x] 10.1 PersistentActorState の実装
  - WaitingRecoveryPermit 状態を定義する
  - RecoveryStarted 状態を定義する
  - Recovering 状態を定義する
  - ProcessingCommands 状態を定義する
  - PersistingEvents 状態を定義する
  - 状態遷移メソッドを実装する（transition_to_recovery_started 等）
  - 不正な遷移でエラーを返す
  - 単体テストを作成する
  - _Requirements: 13.1, 13.2, 13.3, 13.4, 13.5, 13.6, 13.7_

- [x] 10.2 PendingHandlerInvocation の実装
  - StashingHandlerInvocation 構造体を作成する（event, handler, stashing: true）
  - AsyncHandlerInvocation 構造体を作成する（event, handler, stashing: false）
  - PendingHandlerInvocation trait または enum を定義する
  - invoke メソッドでハンドラーを呼び出す
  - is_stashing メソッドでスタッシュ判定を返す
  - 単体テストを作成する
  - _Requirements: 14.1, 14.2, 14.3_

- [x] 10.3 PersistentEnvelope の実装
  - event（ArcShared<dyn Any + Send + Sync>）を保持する
  - sequence_nr を保持する
  - handler を保持する（クロージャまたは関数ポインタ）
  - stashing フラグを保持する
  - into_persistent_repr メソッドで PersistentRepr に変換する
  - 単体テストを作成する
  - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5_

- [x] 11. Eventsourced trait の実装
- [x] 11.1 Eventsourced trait の定義
  - receive_recover メソッドを定義する（リカバリ中のイベント受信）
  - receive_snapshot メソッドを定義する（スナップショットリカバリ処理）
  - receive_command メソッドを定義する（通常コマンド受信）
  - on_recovery_completed メソッドを定義する（リカバリ完了通知）
  - persistence_id メソッドを定義する
  - journal_actor_ref メソッドを定義する
  - snapshot_actor_ref メソッドを定義する
  - recovery メソッドを定義する（デフォルト実装で Recovery::default()）
  - last_sequence_nr メソッドを定義する
  - _Requirements: 15.1, 15.2, 15.3, 15.4, 15.7_

- [x] 11.2 Eventsourced エラーフックの整備
  - on_persist_failure/on_persist_rejected のデフォルト実装を提供する
  - on_recovery_failure/on_snapshot_failure のデフォルト実装を提供する
  - _Requirements: 15.5, 15.6_

- [x] 12. PersistentActorBase の実装
- [x] 12.1 PersistentActorBase の基本構造
  - persistence_id を保持する
  - PersistentActorState を保持する
  - pendingInvocations キュー（VecDeque<PendingHandlerInvocation>）を保持する
  - eventBatch（Vec<PersistentEnvelope>）を保持する
  - journalBatch（Vec<PersistentEnvelope>）を保持する
  - journal_actor_ref を保持する
  - snapshot_actor_ref を保持する
  - current_sequence_nr を保持する
  - _Requirements: 16.1, 16.2, 16.3, 16.4, 16.6_

- [x] 12.2 PersistentActorBase のバッチ管理
  - add_to_event_batch メソッドでイベントをバッチに追加する
  - flush_batch メソッドでバッチを JournalActor に WriteMessages として送信する
  - 送信後、状態を PersistingEvents に遷移する
  - _Requirements: 16.5_

- [x] 12.3 PersistentActorBase のレスポンス処理
  - handle_journal_response メソッドで JournalResponse をディスパッチする
  - WriteMessageSuccess で pendingInvocations からハンドラーを取り出し実行する
  - WriteMessageFailure でエラーハンドラーを呼び出す
  - ReplayedMessage で receive_recover を呼び出す
  - RecoverySuccess で on_recovery_completed を呼び出し、ProcessingCommands に遷移する
  - handle_snapshot_response メソッドで SnapshotResponse をディスパッチする
  - LoadSnapshotResult で receive_snapshot を呼び出す
  - 単体テストを作成する
  - _Requirements: 16.7, 16.8_

- [x] 12.4 PersistentActorBase のリカバリ処理
  - start_recovery メソッドを実装する
  - スナップショットロード → イベントリプレイ → リカバリ完了の流れを実装する
  - Recovery::none() の場合は highest_sequence_nr のみ取得する
  - _Requirements: 16.8_

- [x] 13. PersistentActor trait の実装
- [x] 13.1 PersistentActor trait の定義
  - Eventsourced を継承する
  - base/base_mut を定義する（PersistentActorBase への委譲）
  - start_recovery/handle_journal_response/handle_snapshot_response を定義する
  - _Requirements: 18.1, 18.2, 18.3, 18.4, 18.15, 18.16_

- [x] 13.2 PersistentActor デフォルト実装
  - persist メソッドのデフォルト実装を提供する（PersistentActorBase に委譲）
  - persist_all メソッドのデフォルト実装を提供する
  - persist_async メソッドのデフォルト実装を提供する
  - save_snapshot メソッドのデフォルト実装を提供する
  - delete_messages メソッドのデフォルト実装を提供する
  - delete_snapshots メソッドのデフォルト実装を提供する
  - _Requirements: 18.9, 18.10, 18.11, 18.12, 18.13, 18.14_

- [x] 13.3 PersistentActor リカバリ統合
  - 起動時（pre_start 等）に start_recovery を呼び出すパターンを提供する
  - JournalResponse/SnapshotResponse のディスパッチを handle_* で共通化する
  - リカバリ完了で on_recovery_completed を呼び出し ProcessingCommands に遷移する
  - _Requirements: 18.5, 18.6, 18.7, 18.8_

- [x] 14. AtLeastOnceDelivery の実装
- [x] 14.1 (P) 配信設定型の実装
  - 再配信間隔、最大未確認数、再配信バースト制限を保持する構造体を作成する
  - デフォルト値を提供する
  - _Requirements: 19.3, 19.4_

- [x] 14.2 (P) 未確認配信型の実装
  - delivery_id, destination, payload, sender, timestamp を保持する構造体を作成する
  - Toolbox ジェネリックで ActorRef と TimerInstant を扱う
  - _Requirements: 19.1, 19.5_

- [x] 14.3 (P) 配信状態スナップショット型の実装
  - 現在の delivery_id と未確認配信リストを保持する構造体を作成する
  - リカバリ時の状態復元に使用する
  - _Requirements: 19.5, 19.6_

- [x] 14.4 AtLeastOnceDelivery ジェネリック構造体の実装
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
  - _Requirements: 19.1, 19.2, 19.3, 19.4, 19.5, 19.6, 19.7_

- [x] 15. PersistenceExtension の実装
- [x] 15.1 PersistenceExtension の実装
  - JournalActor と SnapshotActor への ActorRef を保持する構造体を作成する
  - new メソッドで Journal と SnapshotStore を受け取り、対応する Actor を生成する
  - journal_actor メソッドで JournalActor への ActorRef を返す
  - snapshot_actor メソッドで SnapshotActor への ActorRef を返す
  - Clone を実装する
  - Extension trait を実装する
  - 単体テストを作成する
  - _Requirements: 20.1, 20.2, 20.3, 20.4, 20.5_

- [x] 16. モジュール構造と再エクスポートの整備
- [x] 16.1 モジュール構造の整備
  - lib.rs でクレートルートを設定する
  - core.rs で core モジュールエントリを作成する
  - 各コンポーネントを適切なファイルに配置する
  - 2018 モジュール構成に従う（foo.rs + foo/ パターン）
  - mod.rs を使用しない
  - FQCN インポートを使用する
  - ファイル構造:
    - journal.rs, journal_error.rs
    - snapshot_store.rs, snapshot_error.rs
    - in_memory_journal.rs, in_memory_snapshot_store.rs
    - persistent_repr.rs, snapshot.rs, snapshot_metadata.rs, snapshot_selection_criteria.rs
    - recovery.rs
    - journal_message.rs, journal_response.rs
    - snapshot_message.rs, snapshot_response.rs
    - journal_actor.rs, snapshot_actor.rs
    - persistent_actor_state.rs, pending_handler_invocation.rs, persistent_envelope.rs
    - eventsourced.rs, persistent_actor_base.rs, persistent_actor.rs
    - at_least_once_delivery.rs, at_least_once_delivery_config.rs, unconfirmed_delivery.rs, at_least_once_delivery_snapshot.rs
    - persistence_extension.rs
    - persistence_error.rs
  - _Requirements: 28.1, 28.2, 28.3, 28.4, 28.6_

- [x] 16.2 公開 API 再エクスポートの実装
  - 公開 API を core モジュール直下で再エクスポートする
  - 主要な trait と型をエクスポートする
  - ユーザーが import しやすい構造を提供する
  - _Requirements: 28.5_

- [x] 17. no_std ビルド検証
- [x] 17.1 no_std ビルド検証
  - #![no_std] でコンパイルできることを確認する
  - alloc クレートのみに依存していることを確認する
  - std クレートへの依存がないことを確認する
  - cfg-std-forbid lint に違反しないことを確認する
  - THUMB ターゲットでのビルドを確認する
  - _Requirements: 26.1, 26.2, 26.3, 26.4, 26.5_

- [x] 18. Pekko 互換性検証
- [x] 18.1 Pekko 互換性検証
  - Journal の write_messages が Pekko の AsyncWriteJournal と同等のシーケンス番号検証を行うことを確認する
  - SnapshotStore の load_snapshot が Pekko の SnapshotStore と同等の選択ロジックを持つことを確認する
  - Recovery の動作が Pekko の Recovery と同等のセマンティクスを持つことを確認する
  - AtLeastOnceDelivery が Pekko と同等の再配信セマンティクスを持つことを確認する
  - PersistentRepr が Pekko と同等のフィールドを持つことを確認する
  - JournalMessage/JournalResponse が Pekko の journal プロトコルと同等のメッセージ種別を持つことを確認する
  - PersistentActorState が Pekko の Eventsourced の状態遷移と同等のライフサイクルを持つことを確認する
  - PendingHandlerInvocation が Pekko の pendingInvocations と同等のキューイングセマンティクスを持つことを確認する
  - _Requirements: 25.1, 25.2, 25.3, 25.4, 25.5, 25.6, 25.7, 25.8_

- [x] 19. 使用例の作成
- [x] 19.1 基本的な PersistentActor 使用例の作成
  - InMemoryJournal と InMemorySnapshotStore を使用した例を作成する
  - JournalActor と SnapshotActor を起動する
  - メッセージパッシングによるイベントの永続化と再生を示す
  - スナップショットの保存と復元を示す
  - GATsパターンと core::future::ready() の使用方法を示す
  - PersistentActorBase を使用した実装パターンを示す
  - no_std 環境での動作を前提とする
  - _Requirements: 24.1, 24.2, 24.3, 24.5, 24.6, 24.7, 24.8_

- [x] 19.2 AtLeastOnceDelivery 使用例の作成 (fraktor-actor-rs統合後に実装)
  - deliver と confirm_delivery の使用方法を示す
  - handle_message によるRedeliveryTick 処理を示す
  - 配信状態のスナップショットと復元を示す
  - _Requirements: 24.4_

- [x] 20. 統合テストと最終検証
- [x] 20.1 統合テストの作成
  - PersistentActor のリカバリフローをテストする（スナップショット → イベントリプレイ → 完了）
  - メッセージパッシングによる永続化をテストする
  - スナップショットからのリカバリをテストする
  - イベント再生をテストする
  - 状態マシンの遷移をテストする
  - AtLeastOnceDelivery の配信と確認をテストする (fraktor-actor-rs統合後に実装)
  - PersistenceExtension による統合をテストする
  - _Requirements: 23.1, 23.2, 23.3, 23.4, 23.5, 23.6, 23.7, 23.8, 23.9, 23.10, 23.11, 23.12, 23.13, 23.14_

- [x] 20.2 CI チェックの実行
  - scripts/ci-check.sh all を実行する
  - すべてのテストがパスすることを確認する
  - lint エラーがないことを確認する
  - clippy 警告がないことを確認する
  - _Requirements: 26.1, 26.2, 26.3, 26.4, 26.5, 28.1, 28.2, 28.3, 28.4, 28.5, 28.6_
