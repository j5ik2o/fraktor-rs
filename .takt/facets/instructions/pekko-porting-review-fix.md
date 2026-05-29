{extends:fix}

## Pekko porting 固有の補足

### CI失敗からの差し戻し対応（最優先）

`final-ci` ステップから差し戻された場合（= レビューレポートに指摘がないのにこのステップが実行された場合）、
Report Directory内の `final-ci-result.md`、`coder-decisions.md`、または直前のイテレーションログから CI 失敗の詳細を確認し、
**clippy エラー・テスト失敗・ビルドエラーを直接修正すること**。
レビュー指摘がないからといって「修正不要」と判断してはならない。

final-ci 失敗からの差し戻しで reviewer finding が存在しない場合は、本家 `fix` の
`new / reopened` finding 修正、`family_tag` 単位の再発防止テスト追加、複数レビュアー指摘の統合条件は適用しない。
この場合は CI 失敗を直接修正し、対象範囲の lint / 型チェック / テストを再実行できれば完了としてよい。

### 対象レビューレポート

- QAレビュー由来の指摘: `06-qa-review.md`
- Pekko互換性レビュー由来の指摘: `05-pekko-compat-review.md`
- テストレビュー由来の指摘: `07-test-review.md`

### Pekko互換性の指摘への対応

pekko-compat-review の指摘がある場合は、必ず `references/pekko/` の該当ソースを読み直してからRust実装を修正すること。

### 修正完了条件

修正後に必ず以下を実行し、全チェックがパスすることを確認してからレポートに記録すること。

1. 変更範囲に対応する lint / 型チェック（例: `cargo clippy -p <crate> -- -D warnings`）
2. 変更範囲に対応する最小限のテスト（例: `cargo test -p <crate>`）

成功ログをcoder-decisionsレポートの「実行結果」セクションに含めること。
