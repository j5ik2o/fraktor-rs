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
| # | finding_id | 場所 | 問題 | 修正案 |
|---|------------|------|------|--------|
| 1 | PEKKO-NEW-{file}-L{line} | `{file}:{line}` | {問題} | {修正案} |

## 継続指摘（persists）
| # | finding_id | 前回根拠 | 今回根拠 | 問題 | 修正案 |
|---|------------|----------|----------|------|--------|

## 解消済み（resolved）
| finding_id | 解消根拠 |
|------------|----------|

## 出力ルール
- APPROVE → サマリーとPekko API対応状況のみ。REJECT → 該当指摘テーブルのみ
- `new`/`persists` が1件以上 → REJECT。`finding_id` なしの指摘は無効
```
