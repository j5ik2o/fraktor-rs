```markdown
# Issue別コミットログ

## 結果: OK / NG

## Issue別コミット
| Issue | Commit Hash | Commit Message | Conventional準拠 | 英語メッセージ | テスト結果 | issue完了時のci-check.sh all |
|-------|-------------|----------------|------------------|----------------|------------|------------------------------|
| #123 | abcdef1 | fix(remote): avoid duplicate heartbeat probe dispatch (#123) | PASS/FAIL | PASS/FAIL | {コマンドと結果} | PASS/FAIL |

## 未対応Issue（ある場合）
| Issue | 理由 | 必要アクション |
|-------|------|----------------|
| #124 | {理由} | {次アクション} |

## 最終ci-check結果
- `./scripts/ci-check.sh all`: PASS / FAIL
- 実行ログ要約: {1-3行}

## 判定
- 判定: {全issueでコミット作成済みかつci-check全PASS / 未対応issueあり or ci-check失敗あり}
- 理由: {1-3行}
```

**認知負荷軽減ルール**
- OK の場合 30 行以内
- NG の場合 50 行以内
