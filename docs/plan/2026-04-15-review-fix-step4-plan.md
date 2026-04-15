# review-fix step4 計画

## 目的
- 最新の `05-pekko-compat-review.md`、`06-qa-review.md`、`07-test-review.md` を一次情報として確認し、`new` / `reopened` 指摘が残っていないことを確定する
- 前回差し戻し理由だった Report Directory 内の整合性不備が現行ファイルで解消済みかを確認する
- 変更範囲の lint / check / targeted test を再実行し、今回イテレーションの成功証跡を `coder-decisions.md` に記録する

## 実施手順
1. 最新レビュー3本と直前2件の履歴、`summary.md`、`supervisor-validation.md` を確認する
2. `00-plan.md` と `coder-scope.md` が現行実装状態に同期していることを確認する
3. `fraktor-actor-core-rs` を対象に以下を再実行する
   - `./scripts/ci-check.sh ai dylint -m fraktor-actor-core-rs`
   - `cargo check -p fraktor-actor-core-rs --tests --features test-support`
   - `cargo test -p fraktor-actor-core-rs --test kernel_public_surface --features test-support`
   - `cargo test -p fraktor-actor-core-rs behavior_runner_ --features test-support`
   - `cargo test -p fraktor-actor-core-rs dedicated_signal_types_convert_into_behavior_signal_variants --features test-support`
4. 実行結果と収束ゲートを `coder-decisions.md` に反映する

## 想定する更新対象
- `docs/plan/2026-04-15-review-fix-step4-plan.md`
- `.takt/runs/20260415-063831-pekko-phase-actor/reports/coder-decisions.md`

## 完了条件
- 最新レビュー3本が `APPROVE` のままであること
- `00-plan.md` と `coder-scope.md` に stale な記述が残っていないこと
- 上記 lint / check / targeted test がすべて成功すること
- `coder-decisions.md` に今回イテレーションの結果が残ること
