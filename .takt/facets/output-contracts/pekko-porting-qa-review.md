{extends:qa-review}

## Fraktor/Pekko 追加出力要件

- 親の APPROVE 軽量出力ルールより、この Fraktor/Pekko 追加出力要件を優先する
- APPROVE でも次の観点すべてに根拠付き結果を記載する
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
### Fraktor/Pekko 追加観点
| 観点 | 結果 | 備考 |
|------|------|------|
| クレート分離（core / adaptor-std / adaptor-embedded） | ✅ / ❌ | {要点} |
| showcases網羅性 | ✅ / ❌ | {要点} |
| 公開APIの最小化 | ✅ / ❌ | {要点} |
| Dylint lint準拠 | ✅ / ❌ | {要点} |
| rustdocの存在と言語 | ✅ / ❌ | {要点} |
| unsafe使用の妥当性 | ✅ / ❌ | {要点} |
| feature flagの整合性 | ✅ / ❌ | {要点} |
| 依存クレートの妥当性 | ✅ / ❌ | {要点} |
```
