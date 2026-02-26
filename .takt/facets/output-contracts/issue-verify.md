```markdown
# Issue 完了判定

## 結果: PASS / FAIL / BLOCKED

## Issue別受け入れ条件判定
| Issue | 条件 | 判定 | 根拠 |
|-------|------|------|------|
| #123 | {条件} | PASS/FAIL/SKIPPED | {根拠} |

- `SKIPPED` を使う場合は、`issue-commit-log.md` の「理由付きスキップ」と Issue コメント記録を根拠に明記する

## コミット検証
| Issue | コミット数 | Conventional準拠 | 英語メッセージ | issue完了時ci-check | 判定 |
|-------|------------|------------------|----------------|---------------------|------|
| #123 | 1 | PASS/FAIL | PASS/FAIL | PASS/FAIL | PASS/FAIL |

## 最終ci-check判定
| 項目 | 判定 | 根拠 |
|------|------|------|
| `./scripts/ci-check.sh all` | PASS/FAIL | {ログ要約} |

## 未達項目（FAIL/BLOCKED の場合）
| Issue | 不足内容 | 推奨修正 |
|-------|----------|----------|
| #123 | {不足} | {修正案} |

## 最終判断
- 判定: {issue完了条件を満たす / 未達で追加修正が必要 / 判断不能}
- 理由: {1-3行}
```
