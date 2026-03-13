issue の完了条件を満たしているか最終検証してください。

## 手順

1. `00-issue-plan.md` から対象 issue 一覧と受け入れ条件を読み込む
   - 状態が「解決済み」の issue は「既に満たしているか」を確認し、PASS 判定とする
   - 必要に応じて `gh issue view <issue-number>` で Close 済みか確認する
   - 状態が「情報不足」の issue は SKIPPED 判定とする
2. `coder-scope.md` / `coder-decisions.md` / `01-ai-review.md` を確認する
3. issue ごとに受け入れ条件が満たされているかを PASS / FAIL / SKIPPED で判定する
4. issue ごとに `./scripts/ci-check.sh ai all` 実行結果が PASS かを検証する
5. 仕上げとして `./scripts/ci-check.sh ai all` を実行し、最終 PASS を検証する
6. FAIL がある場合は不足点を具体化し、fix に戻すための修正指示をまとめる

## 判定ルール

- 全 issue の全条件が PASS または理由付き SKIPPED で、最終 `./scripts/ci-check.sh ai all` PASS: `issue完了条件を満たす`
- 1件でも FAIL、または理由未記録の SKIPPED がある: `未達で追加修正が必要`
- 判定材料不足: `判断不能`
