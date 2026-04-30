# stream-island-actors 実装計画 5.4-5.5

## 対象タスク

- 5.4 kill switch の `shutdown()` / `abort()` が全 island actor へ伝播することを integration test で固定する。
- 5.5 terminal state の優先度（abort > failure > shutdown/cancel > completion）で graph 全体の状態が導出されることを test で固定する。

## 実装方針

- `Materialized::unique_kill_switch()` / `shared_kill_switch()` は graph 単位の kill switch state を維持する。
- graph kill switch state へ内部通知先を登録し、`shutdown()` / `abort(...)` 時に全 island actor へ command を送る。
- `Abort` command は stream を failure terminal state に遷移させる。
- graph terminal state の導出は公開 API にせず、crate 内部の `MaterializedStreamGraph` に閉じ込める。
- 優先度は `abort > failure > shutdown/cancel > completion > running` とする。

## 検証

- `rtk cargo test -p fraktor-stream-core-rs actor_materializer`
- `rtk rustup run nightly-2025-12-01 cargo fmt --all --check`
- `rtk git diff --check`

TAKT 実行中のため、`./scripts/ci-check.sh ai all` は final-ci ステップまで実行しない。
