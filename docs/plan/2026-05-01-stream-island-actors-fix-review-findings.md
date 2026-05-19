# stream-island-actors レビュー指摘修正計画

## 対象

- change: `stream-island-actors`
- TAKT step: `fix`
- 一次情報: `.takt/runs/20260430-130331-change-stream-island-actors/reports/05-architect-review.md`

## 修正対象

| family_tag | 対応方針 |
|------------|----------|
| `module-boundary` | downstream cancellation control plane を責務名に一致するモジュールへ移し、公開範囲を `core::materialization` 内に閉じる |
| `lifecycle-rollback` | materialization 失敗時の rollback を単一関数に集約し、primary error を維持したまま cleanup error を観測可能にする |
| `dry-violation` | shutdown/cancel の teardown 手順を private 関数に統合し、command 生成だけを差し替える |

## 検証

- `rtk cargo check -p fraktor-stream-core-rs`
- `rtk cargo test -p fraktor-stream-core-rs`
- `rtk git diff --check`

`./scripts/ci-check.sh ai all` は TAKT の `final-ci` ムーブメント専用のため、この fix ステップでは実行しない。
