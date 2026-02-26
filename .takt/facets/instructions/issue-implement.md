`00-issue-plan.md` に基づき、複数 issue を順番に解決してください。

## 手順

1. `00-issue-plan.md` の issue 一覧を順番に処理する
   - 状態が「解決済み」の issue は実装対象から除外する
   - 解決済み issue の場合、plan でコメント/Close 済みであることを確認する
   - 状態が「情報不足」の issue は実装対象から除外する
   - 情報不足 issue の場合、plan でコメント済みであることを確認する
2. 各 issue について、スコープ内ファイルだけを変更する
3. 各 issue について、必要なテスト追加・更新・実行を行う
4. issue ごとに「実装不能」を判定する
   - 実装不能（前提不足・外部依存・再現不能など）の場合は **中断しない**
   - `gh issue comment <issue-number> -b "<日本語コメント>"` で理由・次アクションを記録する
   - `issue-commit-log.md` に「理由付きスキップ」として記録し、次の issue に進む
5. 各 issue の作業完了時に `./scripts/ci-check.sh all` を実行し、PASS を確認する
6. 各 issue の変更を **issue 単位でコミット** する（ただしムーブメント実行ルールで `git commit` が禁止されている場合は除外）
7. 全 issue 処理後に `./scripts/ci-check.sh all` を再実行し、最終 PASS を確認する
8. 全 issue 処理後に受け入れ条件の達成状況を整理する

## 実装方針

- 既存実装パターンに合わせる
- 過剰な抽象化を入れない
- 複数 issue の変更を同一コミットに混ぜない
- issue のコミット前に `git diff --staged` でステージ内容を確認し、対象issue以外が混在していたら分離する
- `./scripts/ci-check.sh all` が PASS するまでコミットしない
- 単一 issue が実装不能でもムーブメント全体を中断しない（理由記録後に継続）

## 実装不能時の記録ルール

- issue コメントは日本語で、最低限「実装不能理由」「不足情報/依存」「次に必要なアクション」を含める
- コメント例:
  - `現時点では {理由} のため実装不能です。{不足情報や依存} の解消後に再開可能です。次アクション: {具体的アクション}。`
- `issue-commit-log.md` には以下を必ず記載する
  - 対象 issue 番号
  - 状態: `理由付きスキップ`
  - Issue へ記録した理由の要約
  - 次アクション

## コミット方針（必須）

- `git commit` が **許可されている場合**:
  - 形式: `<type>(<scope>): <english summary> (#<issue-number>)`
  - 例: `fix(remote): avoid duplicate heartbeat probe dispatch (#227)`
  - `<type>` は `fix|feat|refactor|test|docs|chore` のいずれか
  - メッセージ本文・サマリーは英語で書く（日本語禁止）
  - issue ごとに最低1コミット作成する
  - 各コミットの hash を取得して記録する
  - issue ごとのコミット記録に `./scripts/ci-check.sh all` の結果を含める
- `git commit` が **禁止されている場合**:
  - コミットは作成しない
  - `issue-commit-log.md` に「コミット禁止のため未作成」と明記する
  - 判定は「テスト完了 + コミット禁止のため未作成を明記」でOK扱いとする

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
- {解決済み issue の場合は「対応不要でClose済み」と記載}
- {情報不足 issue の場合は「情報不足でスキップ」と記載}
