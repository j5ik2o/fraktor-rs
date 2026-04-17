# review-fix ステップ実施計画

## 目的

`supervisor-validation.md` の `VAL-NEW-report-plan-routing-mismatch` と `VAL-NEW-report-scope-stale-artifacts` を解消し、Report Directory 内の整合性を最終着地に合わせる。

## 手順

1. `00-plan.md` を見直し、classic routing の未完了項目と現在の完了項目を承認済みレビューに揃える
2. `coder-scope.md` を見直し、削除済み・分割済み成果物名を現行ファイル群へ同期する
3. `fraktor-actor-core-rs` の対象 lint / check / test を再実行し、成功記録を取り直す
4. `coder-decisions.md` に今回の修正内容、検証結果、証拠を反映する
