```markdown
# Pekko互換性レビュー

## 結果: APPROVE / REJECT

## サマリー
{1-2文で結果を要約}

## Pekko API対応状況
| Pekko API | fraktor-rs 対応 | 状態 |
|-----------|----------------|------|
| `ClassName.method` | `type_name::method` | OK / 未実装 / 不正確 |

## 今回の指摘（new）
| # | finding_id | family_tag | 場所 | 問題 | 修正案 |
|---|------------|------------|------|------|--------|
| 1 | PEKKO-NEW-{file}-L{line} | pekko-compat | `{file}:{line}` | {問題} | {修正案} |

## 継続指摘（persists）
| # | finding_id | family_tag | 前回根拠 | 今回根拠 | 問題 | 修正案 |
|---|------------|------------|----------|----------|------|--------|

## 解消済み（resolved）
| finding_id | 解消根拠 |
|------------|----------|

## 再開指摘（reopened）
| # | finding_id | family_tag | 解消根拠（前回） | 再発根拠 | 問題 | 修正案 |
|---|------------|------------|----------------|----------|------|--------|

## 検証証跡
- Pekko参照: {確認対象・確認内容・結果}
- fraktor-rs実装: {確認対象・確認内容・結果}
- テスト: {確認対象・確認内容・結果。未確認ならその旨}

## 出力ルール
- APPROVE → サマリー、Pekko API対応状況、検証証跡を記載
- REJECT → 該当指摘テーブル、Pekko API対応状況、検証証跡を記載
- `new`、`persists`、または `reopened` が1件以上 → REJECT。`finding_id` なしの指摘は無効
```
