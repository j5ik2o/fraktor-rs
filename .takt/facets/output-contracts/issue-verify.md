```markdown
# Issue 完了判定

## 結果

- PASS / FAIL / BLOCKED

## Issue 別判定

- #123: 条件={条件} / 判定={PASS|FAIL|SKIPPED} / 根拠={ログ or テスト}
- SKIPPED は `issue-commit-log.md` と Issue コメントを根拠に明記

## コミット検証

- Issue: #123
- コミット数 / Conventional 準拠 / 英語メッセージ / `ci-check`: `PASS|FAIL`

## 最終判定

- `./scripts/ci-check.sh all`: PASS / FAIL
- 未達時は `Issue` / `不足` / `次アクション` を明示して再オープン
```

**認知負荷軽減ルール**
- PASS: サマリー1行 + 主要根拠のみ
- FAIL: Issue別判定テーブル + 不足条件 + 根本原因 + 修正方針を必須
- BLOCKED: 理由1行 + ブロック要因の成果物参照
