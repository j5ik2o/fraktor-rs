# タスク仕様

## 目的

persistenceモジュールの既存機能を拡張し、不足しているフィールドとユーティリティ（Phase 1-2: trivial〜easy）を追加する。

## 要件

- [ ] `PersistentRepr` に `deleted` フラグと `sender` フィールドを追加する
- [ ] `delete_snapshot` メソッドを実装する（単一スナップショット削除。既存の `delete_snapshots` は範囲削除）
- [ ] `StashOverflowStrategy` enumを実装する（スタッシュ溢れ時の戦略: Drop, Fail）
- [ ] `RecoveryTimedOut` イベント/シグナルを実装する（リカバリタイムアウト通知）
- [ ] `UnconfirmedWarning` を実装する（AtLeastOnceDelivery未確認メッセージの警告）
- [ ] `persist_all_async` を実装する（複数イベントの非フェンス永続化）
- [ ] `defer` / `defer_async` を実装する（永続化完了後の副作用実行）
- [ ] 各機能に対するテストを追加する

## 受け入れ基準

- PersistentReprがPekkoのPersistentReprと同等のフィールドを持つ
- StashOverflowStrategyがスタッシュバッファと統合されている
- defer/defer_asyncで永続化完了後の処理を登録できる
- `./scripts/ci-check.sh all` がパスする

## 参考情報

- ギャップ分析: `docs/gap-analysis/persistence-gap-analysis.md`（カテゴリ2, 3, 6, 7, 8）
- Pekko参照: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/PersistentRepr.scala`
- Pekko参照: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/Eventsourced.scala`
- Pekko参照: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/AtLeastOnceDelivery.scala`
