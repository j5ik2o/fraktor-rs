## GitHub Issue #236: [TAKT] persistence-durable-state (20260224-100011-per-dur)

## 元タスク
- slug: 20260224-100011-per-dur
- task_dir: .takt/tasks/20260224-100011-per-dur
- source: .takt/tasks/20260224-100011-per-dur/order.md

## タスク仕様（order.md）

# タスク仕様

## 目的

persistenceモジュールにDurable State Store（Phase 2-3: easy〜medium）を実装し、CQRS/CRDTベースの状態永続化パターンを提供する。

## 要件

- [ ] `DurableStateStore<A>` traitを実装する（状態の読み書きインターフェース: `getObject`, `upsertObject`, `deleteObject`）
- [ ] `DurableStateUpdateStore<A>` traitを実装する（変更通知付きのDurableStateStore拡張）
- [ ] `DurableStateStoreProvider` traitを実装する（ストア生成のプラグインインターフェース）
- [ ] `DurableStateStoreRegistry` を実装する（プロバイダーの登録・解決）
- [ ] `DurableStateException` エラー型を実装する
- [ ] 各機能に対するテストを追加する

## 受け入れ基準

- DurableStateStoreで状態の読み書きが可能
- DurableStateUpdateStoreで変更ストリームへの通知が可能
- プロバイダーパターンでストア実装を差し替え可能
- `./scripts/ci-check.sh all` がパスする

## 参考情報

- ギャップ分析: `docs/gap-analysis/persistence-gap-analysis.md`（カテゴリ9: Durable State）
- Pekko参照: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/state/DurableStateStore.scala`
- Pekko参照: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/state/DurableStateUpdateStore.scala`
- Pekko参照: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/state/DurableStateStoreProvider.scala`
