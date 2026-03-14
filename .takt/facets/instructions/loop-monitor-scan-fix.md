scan と fix のループが {cycle_count} 回繰り返されました。

各サイクルのレポートを確認し、このループが健全（進捗がある）か、
非生産的（同じ問題を繰り返している）かを判断してください。

**参照するレポート:**
- ロジックバグ: {report:01-logic-scan.md}
- セキュリティ: {report:02-security-scan.md}
- 並行処理: {report:03-concurrency-scan.md}

**判断基準:**
- 前回から問題件数が減少しているか
- 同じ finding_id が persists に残り続けていないか
- 修正が実際にコードに反映されているか
