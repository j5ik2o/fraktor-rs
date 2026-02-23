# タスク仕様

## 目的

PRレビューで指摘された13件を一括で解消し、lazy系実装と周辺コードの整合性を回復する。

## 要件

- [ ] Major指摘（lazy_flow Mat反映不足、LazyFlowLogic副作用消失、backpressure無視、shutdown片側消費、lazy_sink on_start未呼出、collect_values失敗時誤完了）を修正する
- [ ] `use super` から `crate` 参照への修正を反映する
- [ ] `assert` ベース検証を fallible validation に置き換える
- [ ] Minor指摘（rustdoc追加、非rustdocコメント日本語化、use配置修正、defers_factory_callテスト改善）を反映する

## 受け入れ基準

- 対象13件の指摘がすべて解消されている
- lazy系の振る舞いがテストで確認できる
- lint/testが失敗しない状態である

## 参考情報

PR #117 の CodeRabbit 指摘一覧を参照すること。
