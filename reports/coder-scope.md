# 変更スコープ宣言

## タスク
`A4` の新しい `group_by(..., SubstreamCancelStrategy)` 契約に合わせて外部テスト群とトレーサビリティを更新し、利用側から `Drain` / `Propagate` の到達性を固定する。

## 実装計画
1. 対象テスト 3 本とレポート 2 本の現状を確認し、旧契約依存箇所と報告フォーマットを特定する。
2. 新しい `group_by(..., SubstreamCancelStrategy)` 契約に合わせて外部テストとトレーサビリティ記述を更新する。
3. 変更範囲の lint / 型チェックと最小テストを実行し、結果を `reports/coder-scope.md` と `reports/coder-decisions.md` に反映する。

## 変更予定
| 種別 | ファイル |
|------|---------|
| 変更 | `modules/stream/tests/group_by_json_framing_regression.rs` |
| 変更 | `modules/stream/tests/compat_validation.rs` |
| 変更 | `modules/stream/tests/requirement_traceability.rs` |
| 変更 | `reports/coder-scope.md` |
| 変更 | `reports/coder-decisions.md` |

## 推定規模
Small

## 影響範囲
- `modules/stream` の外部 `group_by` 利用契約
- `SubstreamCancelStrategy::Drain` / `SubstreamCancelStrategy::Propagate` の外部到達性
- `modules/stream/tests` の互換性検証と要件トレーサビリティ
