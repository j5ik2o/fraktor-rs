# remote-payload-serialization review/fix plan

## Goal

`openspec/changes/remote-payload-serialization` の proposal / design / spec / tasks をレビューし、論理矛盾と実装 API との明確な不整合がなくなるまで最小限に修正する。

## Steps

1. OpenSpec change と関連する live spec / 実装 API を照合する。
   - verify: `openspec status` / `openspec instructions apply` と対象ファイルの確認。
2. 論理矛盾・用語/API 不整合を特定する。
   - verify: change 内の主張が proposal / design / spec / tasks 間で同じ意味になっていること。
3. 最小限の文書修正を行う。
   - verify: 修正対象行が矛盾解消に直接対応していること。
4. 再レビューと `openspec validate` を実行する。
   - verify: `openspec validate remote-payload-serialization --strict` が成功し、再読で未解消矛盾がないこと。
5. 完了監査を行う。
   - verify: ユーザー要求、成果物、コマンド結果、未実施タスクの扱いを明示して照合する。
