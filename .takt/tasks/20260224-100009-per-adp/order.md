# タスク仕様

## 目的

persistenceモジュールにイベントアダプター機構（Phase 1-3: trivial〜medium）を実装し、イベントスキーマの進化と後方互換性を可能にする。

## 要件

- [ ] `WriteEventAdapter` traitを実装する（永続化前のイベント変換: `toJournal`）
- [ ] `ReadEventAdapter` traitを実装する（読み出し後のイベント変換: `fromJournal`）
- [ ] `EventSeq` 型を実装する（1つのイベントから複数イベントへの展開）
- [ ] `Tagged` 型を実装する（イベントへのタグ付け）
- [ ] `EventAdapters` レジストリを実装する（イベント型→アダプターのマッピング管理）
- [ ] `IdentityEventAdapter` を実装する（何もしないデフォルトアダプター）
- [ ] `PersistentRepr` に `adapters` フィールドを追加する
- [ ] 各機能に対するテストを追加する

## 受け入れ基準

- WriteEventAdapterでイベントの永続化形式を変換できる
- ReadEventAdapterで読み出し時のイベント復元ができる
- Taggedでイベントにタグを付与し、タグベースのクエリが可能
- EventAdaptersレジストリでイベント型に基づくアダプター解決ができる
- `./scripts/ci-check.sh all` がパスする

## 参考情報

- ギャップ分析: `docs/gap-analysis/persistence-gap-analysis.md`（カテゴリ4: イベントアダプター）
- Pekko参照: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/EventAdapter.scala`
- Pekko参照: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/Tagged.scala`
- Pekko参照: `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/EventAdapters.scala`
