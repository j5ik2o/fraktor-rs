# 時間依存テスト対応コミット分割プラン

更新日: 2026-03-18

## 概要

今回の変更は、以下の 2 コミットに分ける。

1. 時間依存テスト対応タスクの実装本体
2. TAKT の実行記録とタスク状態

コード変更と TAKT 記録は責務が異なるため、履歴を分ける。

## コミット 1

### 目的

時間依存テスト対応タスクの実装本体だけを残す。

### 含めるファイル

- `modules/actor/Cargo.toml`
- `modules/actor/src/std/pattern/circuit_breaker.rs`
- `modules/actor/src/std/pattern/circuit_breaker/tests.rs`
- `modules/actor/src/std/pattern/circuit_breaker_shared.rs`
- `modules/actor/src/std/pattern/circuit_breaker_shared/tests.rs`
- `modules/actor/src/std/scheduler/tick/tests.rs`
- `modules/actor/src/std/system/base/tests.rs`
- `modules/remote/Cargo.toml`
- `modules/remote/src/std/endpoint_transport_bridge/tests.rs`
- `modules/remote/tests/multi_node_scenario_integration.rs`
- `modules/remote/tests/quickstart.rs`
- `scripts/ci-check.sh`

### 含めないファイル

- `modules/actor/src/std.rs`
- `modules/actor/src/std/actor.rs`
- `modules/actor/src/std/dispatch.rs`
- `modules/actor/src/std/dispatch/dispatcher.rs`
- `modules/actor/src/std/event.rs`
- `modules/actor/src/std/event/logging.rs`
- `modules/actor/src/std/event/stream.rs`
- `modules/actor/src/std/props.rs`
- `modules/actor/src/std/scheduler.rs`
- `modules/actor/src/std/system.rs`
- `modules/actor/src/std/tests.rs`
- `modules/actor/src/std/typed.rs`
- `modules/actor/src/std/typed/actor.rs`

### 推奨コミットメッセージ

`refactor(actor/remote): make time-dependent tests deterministic`

## コミット 2

### 目的

この TAKT タスクの状態記録を残す。

### 含めるファイル

- `.takt/tasks.yaml`
- `.takt/tasks/20260317-133028-2026-03-17/` 配下
- `.takt/runs/20260317-133028-2026-03-17/` 配下

### 含めないファイル

- `docs/plans/time-dependent-test-refactoring-plan.md`
- `docs/plans/time-dependent-test-commit-plan.md`
- `docs/plans/actor-std-wrapper-cleanup-plan.md`

### 推奨コミットメッセージ

`chore(takt): record completion of time-dependent test task`

## 補足

- `references/okite-ai`, `references/takt` の submodule 変更は今回のコミット対象外
- gap-analysis の更新は今回のコミット対象外
- `modules/actor/src/std.rs` 周辺の wrapper 整理も今回のコミット対象外
