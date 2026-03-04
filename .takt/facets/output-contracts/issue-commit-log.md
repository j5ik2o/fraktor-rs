```markdown
# Issue別コミットログ

## 結果: OK / NG

## Issue別コミット
| Issue | 状態 | Commit Hash | Commit Message |
|-------|------|-------------|----------------|
| #123 | 実装完了 / スキップ | abcdef1 | fix(remote): ... (#123) |

## 最終ci-check結果
- `./scripts/ci-check.sh all`: PASS / FAIL

## 判定
- 判定: {全issueコミット完了 / 失敗あり}
- 理由: {1-3行}
```

**認知負荷軽減ルール**
- OK の場合 30 行以内
- NG の場合 50 行以内
