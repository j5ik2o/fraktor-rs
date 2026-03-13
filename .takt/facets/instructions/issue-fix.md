AIレビュー指摘および検証差分に基づき、issue ごとに修正してください。

## 手順

1. `00-issue-plan.md` から対象 issue 一覧を読み込む
2. `01-ai-review.md` を読み込む
3. `02-issue-verify.md` が存在する場合のみ追加で読み込む
4. issue ごとに指摘内容を整理し、必要な修正だけを実施する
5. 修正した issue ごとにテストを実行する
6. issue ごとに「修正不能」を判定する
   - 修正不能（再現不能・前提不足・外部依存など）の場合は **中断しない**
   - `gh issue comment <issue-number> -b "<日本語コメント>"` で理由・次アクションを記録する
   - 次の issue に進む
7. 修正した issue ごとに `./scripts/ci-check.sh ai all` を実行し、PASS を確認する
8. 全修正後に `./scripts/ci-check.sh ai all` を再実行し、最終 PASS を確認する

## 修正原則

- 指摘に直接関係する修正だけを行う
- 既存の実装パターン・規約に合わせる
- `./scripts/ci-check.sh ai all` が PASS するまで次の issue に進まない
- 単一 issue が修正不能でもムーブメント全体を中断しない（理由記録後に継続）

## 判定基準

- 全 issue が「修正完了」または「理由をissueに記録してスキップ」: `修正完了`
- 1件でも修正不能かつ理由未記録の issue がある: `修正不能なissueあり`
- 判定材料が不足: `判断不能`
