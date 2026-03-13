# Issue 単位コミット

実装・修正済みの変更を issue 単位でコミットしてください。

## 手順

1. `00-issue-plan.md` から対象 issue 一覧を読み込む
2. `coder-scope.md` から issue ごとの対象ファイルを読み込む
3. `git status` で未コミットの変更一覧を確認する
4. issue ごとに関連ファイルを `git add` してコミットする
   - 状態が「解決済み」「情報不足」「理由付きスキップ」の issue はコミット不要
   - 複数 issue の変更を同一コミットに混ぜない
   - `git diff --staged` でステージ内容を確認し、対象 issue 以外が混在していたら分離する
5. コミットメッセージ形式: `<type>(<scope>): <english summary> (#<issue-number>)`
   - `<type>` は `fix|feat|refactor|test|docs|chore` のいずれか
   - メッセージは英語で書く（日本語禁止）
6. `./scripts/ci-check.sh ai all` を実行し、最終 PASS を確認する

## コミット禁止時の対応

- ムーブメント実行ルールで `git commit` が禁止されている場合はコミットを作成しない
- `issue-commit-log.md` に「コミット禁止のため未作成」と明記する
- 判定は「コミット禁止のため未作成を明記」で OK 扱いとする

## 必須出力（見出しを含める）

## Issue別コミット

- {issue番号, commit hash, commit message}
- {解決済み issue は「対応不要」と記載}
- {スキップ issue は「理由付きスキップ」と記載}

## 最終ci-check結果

- `./scripts/ci-check.sh ai all`: PASS / FAIL
