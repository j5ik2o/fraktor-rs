{extends:supervise}

## Pekko porting 固有の補足

Pekko 互換の見た目ではなく、Rust / fraktor-rs の設計原則を壊さずに
Pekko の契約意図が実現されているかを最終判定すること。

## fraktor-rs 固有の検証

- review-fix / implement ステップのレポートに、変更範囲に対する検証（dylint/対象テスト）が記録されていることを確認
- `06-qa-review.md` の判定がapprovedであることを確認
- `05-pekko-compat-review.md` の承認済み判定があることを確認
- `07-test-review.md` の承認済み判定があることを確認
- `review-fix` の修正内容が上記レビュー結果と矛盾していないことを確認
- wrapper / alias 偽装、fallback/no-op 公開API、public/internal 境界悪化がレポート上で否定されていることを確認
- supervisor は CI を自身では実行しない（implement/qa-fix/pekko-compat-fix の実行済み証跡をレポートで確認するのみ）
- `./scripts/ci-check.sh ai all` は supervise の次の `final-ci` ステップで1回だけ実行する
