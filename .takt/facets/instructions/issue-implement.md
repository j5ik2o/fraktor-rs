`00-issue-plan.md` に基づき、複数 issue を順番に解決してください。

## 手順

1. `00-issue-plan.md` の issue 一覧を順番に処理する
2. 各 issue について、スコープ内ファイルだけを変更する
3. 各 issue について、必要なテスト追加・更新・実行を行う
4. 各 issue の作業完了時に `./scripts/ci-check.sh all` を実行し、PASS を確認する
5. 各 issue の変更を **issue 単位でコミット** する
6. 全 issue 処理後に `./scripts/ci-check.sh all` を再実行し、最終 PASS を確認する
7. 全 issue 処理後に受け入れ条件の達成状況を整理する

## 実装方針

- 既存実装パターンに合わせる
- 過剰な抽象化を入れない
- 複数 issue の変更を同一コミットに混ぜない
- issue のコミット前に `git diff --staged` でステージ内容を確認し、対象issue以外が混在していたら分離する
- `./scripts/ci-check.sh all` が PASS するまでコミットしない

## コミット方針（必須）

- 形式: `<type>(<scope>): <english summary> (#<issue-number>)`
- 例: `fix(remote): avoid duplicate heartbeat probe dispatch (#227)`
- `<type>` は `fix|feat|refactor|test|docs|chore` のいずれか
- メッセージ本文・サマリーは英語で書く（日本語禁止）
- issue ごとに最低1コミット作成する
- 各コミットの hash を取得して記録する
- issue ごとのコミット記録に `./scripts/ci-check.sh all` の結果を含める

## 必須出力（見出しを含める）

## 受け入れ条件への対応
- {条件ごとの対応結果}

## 変更内容
- {issue ごとにファイル単位で要約}

## テスト結果
- {issue ごとの実行コマンドと結果}
- {issue ごとの `./scripts/ci-check.sh all` 実行結果}
- {最終 `./scripts/ci-check.sh all` 実行結果}

## コミット結果
- {issue番号, commit hash, commit message}
