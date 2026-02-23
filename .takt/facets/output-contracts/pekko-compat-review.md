```markdown
# Pekko互換性レビュー

## 結果: APPROVE / REJECT

## サマリー
{1-2文で結果を要約}

## Pekko API対応状況
| Pekko API | fraktor-rs 対応 | 状態 |
|-----------|----------------|------|
| `ClassName.method` | `type_name::method` | OK / 未実装 / 不正確 |

## 確認した観点
- [x] Pekko参照実装との対応関係
- [x] Scala→Rust変換パターン
- [x] 型パラメータ（TB: RuntimeToolbox）
- [x] no_std/std分離
- [x] CQS原則
- [x] 命名規約
- [x] テストカバレッジ
- [x] YAGNI

## 今回の指摘（new）
| # | finding_id | 場所 | 問題 | 修正案 |
|---|------------|------|------|--------|
| 1 | PEKKO-NEW-src-file-L42 | `src/file.rs:42` | 問題の説明 | 修正方法 |

## 継続指摘（persists）
| # | finding_id | 前回根拠 | 今回根拠 | 問題 | 修正案 |
|---|------------|----------|----------|------|--------|
| 1 | PEKKO-PERSIST-src-file-L77 | `src/file.rs:77` | `src/file.rs:77` | 未解消 | 既存修正方針を適用 |

## 解消済み（resolved）
| finding_id | 解消根拠 |
|------------|----------|
| PEKKO-RESOLVED-src-file-L10 | 修正済み |

## REJECT判定条件
- `new` または `persists` が1件以上ある場合のみ REJECT 可
- `finding_id` なしの指摘は無効
```

**認知負荷軽減ルール:**
- APPROVE → サマリーとPekko API対応状況のみ
- REJECT → 該当指摘のみ表で記載
