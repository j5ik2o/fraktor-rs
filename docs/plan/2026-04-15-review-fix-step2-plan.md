# review-fix ステップ実施計画

## 目的

`.takt/runs/20260415-063831-pekko-phase-actor/reports/` 配下の最新レビューレポートと直前履歴を確認し、`new` / `reopened` 指摘および CI 差し戻し要因を修正する。

## 手順

1. レビュー報告とポリシー・知識を確認し、`family_tag` ごとに `new` / `reopened` / `persists` を整理する
2. 該当する既存実装と Pekko 参照実装を確認し、既存パターンに沿った修正方針を固める
3. 必要なコードとテストを修正・追加し、同一 `family_tag` の潜在箇所も同時に修正する
4. 変更範囲の `clippy` とテストを実行し、結果を `coder-decisions.md` に記録する
