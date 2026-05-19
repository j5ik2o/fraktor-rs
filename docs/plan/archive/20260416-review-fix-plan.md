# review-fix 計画メモ

## 対象

- TAKT ワークフロー `pekko-porting`
- ステップ `review-fix`
- 対象レポート
  - `05-pekko-compat-review.md`
  - `06-qa-review.md`
  - `07-test-review.md`

## 実施手順

1. 対象レポートと直近履歴を確認し、`family_tag` ごとの指摘を整理する。
2. 指摘に対応する既存実装と Pekko 参照実装を確認し、修正対象を確定する。
3. 指摘内容と同じ `family_tag` の潜在箇所をまとめて修正する。
4. `family_tag` ごとに最低 1 件の再発防止テストを追加する。
5. 変更範囲の lint / 型チェックと最小限のテストを実行する。
6. 実行結果と証拠を `coder-decisions.md` に記録する。

## 注意事項

- `review-fix` ステップでは `./scripts/ci-check.sh ai all` を実行しない。
- Pekko 互換性指摘は `references/pekko/` の該当ソースを確認してから修正する。
- 後方互換は考慮せず、現行設計に合わせて最小かつ正確に直す。
