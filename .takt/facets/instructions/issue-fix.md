AIレビュー指摘および検証差分に基づき、issue ごとに修正してください。

## 手順

1. `00-issue-plan.md` から対象 issue 一覧を読み込む
2. `01-ai-review.md` と `issue-commit-log.md` を読み込む
3. `02-issue-verify.md` が存在する場合のみ追加で読み込む
4. issue ごとに指摘内容を整理し、必要な修正だけを実施する
5. 修正した issue ごとにテストを実行する
6. issue ごとに「修正不能」を判定する
   - 修正不能（再現不能・前提不足・外部依存など）の場合は **中断しない**
   - `gh issue comment <issue-number> -b "<日本語コメント>"` で理由・次アクションを記録する
   - `issue-commit-log.md` に「理由付きスキップ」として記録し、次の issue に進む
7. 修正した issue ごとに `./scripts/ci-check.sh all` を実行し、PASS を確認する
8. 修正を issue 単位でコミットする（ただしムーブメント実行ルールで `git commit` が禁止されている場合は除外）
9. issue ごとのコミット結果を更新する
10. 全修正後に `./scripts/ci-check.sh all` を再実行し、最終 PASS を確認する

## 修正原則

- 指摘に直接関係する修正だけを行う
- 複数 issue の変更を1コミットに混在させない
- 既存の実装パターン・規約に合わせる
- `./scripts/ci-check.sh all` が PASS するまでコミットしない
- 単一 issue が修正不能でもムーブメント全体を中断しない（理由記録後に継続）

## コミット方針（必須）

- `git commit` が **許可されている場合**:
  - 形式: `<type>(<scope>): <english summary> (#<issue-number>)`
  - `<type>` は `fix|feat|refactor|test|docs|chore` のいずれか
  - コミットメッセージは英語で書く（日本語禁止）
  - 各 issue で新規変更がある場合は必ずコミットを作成する
  - issue ごとのコミット記録に `./scripts/ci-check.sh all` の結果を含める
- `git commit` が **禁止されている場合**:
  - コミットは作成しない
  - `issue-commit-log.md` に「コミット禁止のため未作成」と明記する
  - 判定は「テスト完了 + コミット禁止のため未作成を明記」でOK扱いとする

## 判定基準

- 全 issue が「修正完了」または「理由をissueに記録してスキップ」され、必要コミット作成完了: `修正完了かつ必要なissue単位コミット完了`
- 1件でも修正不能かつ理由未記録の issue がある: `修正不能なissueあり`
- 判定材料が不足: `判断不能`
