```markdown
# アーキテクチャレビュー

## 結果: APPROVE / NEEDS_FIX

## サマリー
{1-3文で、Phase完了と構造品質を分けて要約する}

## 判定
| 項目 | 結果 | 根拠 |
|------|------|------|
| Phase 完了 | ✅ / ❌ | {plan 実施状況} |
| 構造品質ゲート | ✅ / ❌ | {高凝集・低結合の観点} |
| 総合判定 | APPROVE / NEEDS_FIX | {1文} |

## 構造品質ゲート
| 観点 | 結果 | 根拠 |
|------|------|------|
| 高凝集 | ✅ / ❌ | {変更理由が閉じているか} |
| 低結合 | ✅ / ❌ | {依存方向・依存数} |
| 過度なフラット配置の解消 | ✅ / ❌ | {root/親階層の改善} |
| future placement predictability | ✅ / ❌ | {新規追加時の置き場が一意か} |
| migration integrity | ✅ / ❌ | {旧互換層なし・実ファイル移動} |

## 構造改善メトリクス
| 指標 | Before | After | 判定 | 備考 |
|------|--------|-------|------|------|
| root 直下の責務数 | {数} | {数} | ✅ / ❌ | |
| 代表的 dumping ground の責務数 | {数} | {数} | ✅ / ❌ | |
| 代表的依存集中点の import / 依存先数 | {数} | {数} | ✅ / ❌ | |
| 追加先が曖昧な責務カテゴリ数 | {数} | {数} | ✅ / ❌ | |

## 配置予測チェック
| 代表責務 | 追加先 | 一意に定まるか | 根拠 |
|----------|--------|----------------|------|
| {例} | {パス} | ✅ / ❌ | {理由} |
| {例} | {パス} | ✅ / ❌ | {理由} |
| {例} | {パス} | ✅ / ❌ | {理由} |

## 今回の指摘（new）
| # | finding_id | family_tag | スコープ | 場所 | 問題 | 修正案 |
|---|------------|------------|---------|------|------|--------|
| 1 | ARCH-NEW-src-file-L42 | cohesion | スコープ内 | `src/file.rs:42` | 問題の説明 | 修正方法 |

family_tag: `cohesion` / `coupling` / `discoverability` / `layering` / `migration-integrity`

## 継続指摘（persists）
| # | finding_id | family_tag | 前回根拠 | 今回根拠 | 問題 | 修正案 |
|---|------------|------------|----------|----------|------|--------|
| 1 | ARCH-PERSIST-src-file-L77 | coupling | `src/file.rs:77` | `src/file.rs:77` | 未解消 | 既存修正方針を適用 |

## 解消済み（resolved）
| finding_id | 解消根拠 |
|------------|----------|
| ARCH-RESOLVED-src-file-L10 | `src/file.rs:10` は構造品質ゲートを満たす |

## 再開指摘（reopened）
| # | finding_id | family_tag | 解消根拠（前回） | 再発根拠 | 問題 | 修正案 |
|---|------------|------------|----------------|---------|------|--------|
| 1 | ARCH-REOPENED-src-file-L55 | discoverability | `前回: src/file.rs:10 で修正済み` | `src/file.rs:55 で再発` | 問題の説明 | 修正方法 |

## NEEDS_FIX 判定条件
- `new`、`persists`、または `reopened` が1件以上ある場合のみ NEEDS_FIX 可
- `finding_id` なしの指摘は無効
- `Phase 完了 = ✅` かつ `構造品質ゲート = ❌` でも NEEDS_FIX にする
```
