# タスク仕様

## 目的

PR #117 のレビュー指摘127件を解消し、TAKT設定・ドキュメント・streams/persistence実装の整合性を回復する。

## 要件

- [ ] 指摘127件（Critical 8 / Major 32 / Minor 64 / Trivial 14 / その他 9）を分類し、修正方針を明確化する
- [ ] `.takt/pieces/*.yaml` の参照整合、knowledge定義、loop monitorの不整合を修正する
- [ ] `.takt/facets/instructions/*.md` の曖昧手順・出力先未定義を解消する
- [ ] `.takt/facets/knowledge/*.md` と `.agent/skills/*.md` の markdownlint 指摘（MD022/MD031/MD040/MD041/MD058）を解消する
- [ ] `modules/streams/src/core/stage/flow.rs`、`modules/persistence/src/core/persistence_context.rs`、`modules/streams/src/core/hub/partition_hub.rs` の主要指摘を解消する
- [ ] 修正後に関連テスト・lintが通る状態にする

## 受け入れ基準

- 指摘一覧に対して未対応項目が残っていない
- 設定参照不整合・実行不能要因が解消されている
- 対象モジュールの動作とテスト結果に退行がない

## 参考情報

PR #117 のレビューコメント（CodeRabbit）と対象ファイル差分を参照すること。
