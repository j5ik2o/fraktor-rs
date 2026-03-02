## GitHub Issue #235: [TAKT] persistence-enhancements (20260224-100010-per-enh)

## 元タスク
- slug: 20260227-131924-github-issue-235-takt-persistence
- task_dir: .takt/tasks/20260227-131924-github-issue-235-takt-persistence
- source: .takt/tasks/20260224-100010-per-enh/order.md

## タスク仕様（order.md）

# タスク仕様

## 目的

persistenceモジュールの既存機能を拡張し、不足しているフィールドとユーティリティ（Phase 1-2: trivial〜easy）を追加する。

## 要件

### Phase 1（先行実装・先にマージ）

- [ ] `PersistentRepr` に新規フィールド `deleted` と `sender` を追加する（既存フィールド `persistence_id`, `sequence_nr`, `payload`, `manifest`, `writer_uuid` は変更対象外）
- [ ] `delete_snapshot` メソッドを実装する（単一スナップショット削除。既存の `delete_snapshots` は範囲削除）
- [ ] `PersistentRepr` 新規フィールドと `delete_snapshot` のユニットテストを追加する

### Phase 2（設計レビュー完了後に着手する確定項目）

- [ ] 既存の `persist_all` と連携する `defer_async` を実装する（同期版 `defer` は対象外）
- [ ] Phase 2対象のユニットテストを追加する

### 確認待ち（ステークホルダー承認後に着手）

- [ ] `StashOverflowStrategy` を今回スコープに含めるか確認する
- [ ] `RecoveryTimedOut` を今回スコープに含めるか確認する
- [ ] `UnconfirmedWarning` を今回スコープに含めるか確認する
- [ ] `persist_all_async` は「design review pending」のフォローアップタスクとして扱う
- [ ] 同期版 `defer` をフォローアップタスクに分離する

## 受け入れ基準

- Phase 1 と Phase 2 を別PRまたは別タスクとして管理し、Phase 1 を先にマージする
- `PersistentRepr` の既存フィールド（`persistence_id`, `sequence_nr`, `payload`, `manifest`, `writer_uuid`）が維持される
- `PersistentRepr` の新規フィールド（`deleted`, `sender`）が追加される
- `delete_snapshot` は「保存→読み込み→単一削除」の順で検証し、対象のみ削除される（受け入れテスト: `core::persistent_actor::tests::persistent_actor_delete_snapshot_sends_message`）
- `RecoveryTimedOut` はスコープに含める場合のみ、発火条件・遷移・処理結果を検証する（受け入れテスト候補: `core::persistent_actor_adapter::tests::adapter_forwards_recovery_timed_out_signal`）
- `UnconfirmedWarning` はスコープに含める場合のみ、発火条件と観測方法（ログ/イベント/メトリクス）を検証する（受け入れテスト候補: `core::unconfirmed_warning::tests::unconfirmed_warning_reports_count`）
- `persist_all_async` はスコープ承認後のみ、順序非保証・ackセマンティクスを検証し、`core::persistent_actor::tests::persistent_actor_persist_all_async_increments_sequence` などの関連テストが通る
- `persist_all_async` 性能評価は以下の前提を明記して実施する（ローカル参考値）:
  - 実行環境: CPU/メモリ、OS、ストレージバックエンド、ネットワーク条件、ランタイム設定
  - 測定方法: ウォームアップあり、5回測定、平均/中央値/標準偏差を記録
  - 許容分散: 最大変動率を明記し、CIでは定常ベースライン比較を行う
- `./scripts/ci-check.sh all` がパスする

## 設計メモ（Rust固有）

- `defer_async` / `defer` のコールバックは借用ではなく所有権を `move` で受け取り、コールバック寿命は `Send + 'static` を前提に設計する
- `ArcShared` は `fraktor_utils_rs::core::sync::ArcShared` で、`std::sync::Arc` 相当のプロジェクト固有共有型として扱う
- `sender` や `ActorRef` は短命参照を保持せず、`ArcShared` などの所有可能な型で保持する
- `Drop` trait は非同期副作用実行トリガーとして使わない。終了処理は明示的な完了API（`Future` / completion message）で扱う
- 移植方針は「Pekko互換を盲目的に再現しない」。`persist_all` と `defer_async` を先行し、`persist_all_async` と同期版 `defer` は設計レビュー完了まで保留する
- 比較対象シンボル: `persist_all_async`, `persist_all`, `defer`, `defer_async`, `Drop`

## 参考情報

- ギャップ分析: `docs/gap-analysis/persistence-gap-analysis.md`（カテゴリ2, 3, 6, 7, 8）
- Tokio async/await パターン整理: `tokio::spawn`, `Pin<Box<dyn Future<...>>>`, `select!` の使い分けを設計レビューで確認する
- 所有権/借用メモ: `sender` と `ActorRef` の寿命境界（借用禁止・所有前提）を明文化する
- Rust参照（比較用）: `tokio`, `bastion`, `raft-rs` の非同期・永続化設計パターン
- Pekko参照（比較材料のみ）: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/PersistentRepr.scala`
- Pekko参照（比較材料のみ）: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Eventsourced.scala`
- Pekko参照（比較材料のみ）: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/AtLeastOnceDelivery.scala`
- `defer` と `sender` を含む変更は実装前に設計レビューを必須化する


### Labels
takt
