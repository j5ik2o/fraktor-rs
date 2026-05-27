{extends:qa-review}

## Fraktor/Pekko 追加出力要件

- 親の APPROVE 軽量出力ルールより、この Fraktor/Pekko 追加出力要件を優先する
- APPROVE でも次の観点すべてに根拠付き結果を記載する
  - テストカバレッジ
  - テスト品質
  - エラーハンドリング
  - ドキュメント
  - 保守性
  - クレート分離（core / adaptor-std / adaptor-embedded）
  - showcases網羅性
  - 公開APIの最小化
  - Dylint lint準拠
  - rustdocの存在と言語
  - unsafe使用の妥当性
  - feature flagの整合性
  - 依存クレートの妥当性
- 根拠には Rust のファイル/行、Cargo.toml、showcases、または実行済みコマンド結果を示す

```markdown
### Fraktor/Pekko 評価項目詳細（根拠必須）
| 観点 | 結果 | 備考 |
|------|------|------|
| テストカバレッジ | ✅ / ❌ | {ファイル/行または検索結果} |
| テスト品質 | ✅ / ❌ | {ファイル/行または検索結果} |
| エラーハンドリング | ✅ / ❌ | {ファイル/行または検索結果} |
| ドキュメント | ✅ / ❌ | {ファイル/行または検索結果} |
| 保守性 | ✅ / ❌ | {ファイル/行または検索結果} |
| クレート分離（core / adaptor-std / adaptor-embedded） | ✅ / ❌ | {要点} |
| showcases網羅性 | ✅ / ❌ | {要点} |
| 公開APIの最小化 | ✅ / ❌ | {要点} |
| Dylint lint準拠 | ✅ / ❌ | {要点} |
| rustdocの存在と言語 | ✅ / ❌ | {要点} |
| unsafe使用の妥当性 | ✅ / ❌ | {要点} |
| feature flagの整合性 | ✅ / ❌ | {要点} |
| 依存クレートの妥当性 | ✅ / ❌ | {要点} |

## REJECT判定条件
- `new`、`persists`、または `reopened` が1件以上ある場合のみ REJECT 可
- `finding_id` なしの指摘は無効
- 上記13観点すべてに根拠付き結果がない場合は REJECT
```
