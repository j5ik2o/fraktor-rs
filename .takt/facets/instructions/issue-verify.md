issue の完了条件を満たしているか最終検証してください。

## 手順

1. `00-issue-plan.md` から対象 issue 一覧と受け入れ条件を読み込む
   - 状態が「解決済み」の issue は「既に満たしているか」を確認し、PASS 判定とする
   - 必要に応じて `gh issue view <issue-number>` で Close 済みか確認する
   - 状態が「情報不足」の issue は SKIPPED 判定とする
2. `coder-scope.md` / `coder-decisions.md` / `01-ai-review.md` / `issue-commit-log.md` を確認する
3. issue ごとに受け入れ条件が満たされているかを PASS / FAIL / SKIPPED で判定する
   - `issue-commit-log.md` に「理由付きスキップ」があり、Issueコメント記録が確認できる場合は SKIPPED を許可する
4. issue ごとにコミット要件を満たすか検証する（`git commit` が禁止されている場合はコミット検証をスキップし、理由の明記を確認する）
5. issue ごとに `./scripts/ci-check.sh all` 実行結果が PASS かを検証する
6. 仕上げとして `./scripts/ci-check.sh all` を実行し、最終 PASS を検証する
7. FAIL がある場合は不足点を具体化し、fix に戻すための修正指示をまとめる

## コミット検証ルール

- `git commit` が **許可されている場合**:
  - 各 issue で `(#<issue-number>)` を含むコミットが1件以上あること
  - コミットメッセージが Conventional Commits 形式であること
  - コミットメッセージに日本語が含まれないこと（英語メッセージ）
  - 1つのコミットに複数 issue を混在させていないこと
  - 各 issue で `./scripts/ci-check.sh all` が PASS していること
- `git commit` が **禁止されている場合**:
  - `issue-commit-log.md` に「コミット禁止のため未作成」が明記されていること
  - 各 issue で `./scripts/ci-check.sh all` が PASS していること

## 判定ルール

- 全 issue の全条件が PASS または理由付き SKIPPED で、全 issue のコミット要件 PASS（またはコミット禁止のため未作成を明記）かつ最終 `./scripts/ci-check.sh all` PASS: `issue完了条件を満たす`
- 1件でも FAIL、または理由未記録の SKIPPED がある: `未達で追加修正が必要`
- 判定材料不足: `判断不能`
