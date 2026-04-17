# review-fix step11 レポート整合計画

## 目的

- `final-ci` 差し戻しで指摘された Report Directory 内の不整合を解消する
- `00-plan.md` と `coder-scope.md` を現行成果物へ同期する
- 検証コマンドを再実行し、`coder-decisions.md` に成功ログを反映する

## 実施手順

1. 差し戻しレポートと現行成果物を突き合わせ、ズレの有無を確認する
2. `00-plan.md` に未実装タスクと今回収束済みタスクの境界を明記する
3. `coder-scope.md` に現行 fixture 構成と除外済み成果物を明記する
4. 変更範囲の lint / check / targeted test を再実行する
5. `coder-decisions.md` に今回の修正内容と実行結果を記録する
