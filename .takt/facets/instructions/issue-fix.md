AIレビュー指摘および検証差分に基づき、issue ごとに修正してください。

## 手順

1. `00-issue-plan.md` から対象 issue 一覧を読み込む
2. `01-ai-review.md` と `02-issue-verify.md` と `issue-commit-log.md` を読み込む
3. issue ごとに指摘内容を整理し、必要な修正だけを実施する
4. 修正した issue ごとにテストを実行する
5. 修正した issue ごとに `./scripts/ci-check.sh all` を実行し、PASS を確認する
6. 修正を issue 単位でコミットする
7. issue ごとのコミット結果を更新する
8. 全修正後に `./scripts/ci-check.sh all` を再実行し、最終 PASS を確認する

## 修正原則

- 指摘に直接関係する修正だけを行う
- 複数 issue の変更を1コミットに混在させない
- 既存の実装パターン・規約に合わせる
- `./scripts/ci-check.sh all` が PASS するまでコミットしない

## コミット方針（必須）

- 形式: `<type>(<scope>): <english summary> (#<issue-number>)`
- `<type>` は `fix|feat|refactor|test|docs|chore` のいずれか
- コミットメッセージは英語で書く（日本語禁止）
- 各 issue で新規変更がある場合は必ずコミットを作成する
- issue ごとのコミット記録に `./scripts/ci-check.sh all` の結果を含める

## 判定基準

- 全 issue の必要修正完了かつ必要コミット作成完了: `修正完了かつ必要なissue単位コミット完了`
- 1件でも修正不能な issue がある: `修正不能なissueあり`
- 判定材料が不足: `判断不能`
