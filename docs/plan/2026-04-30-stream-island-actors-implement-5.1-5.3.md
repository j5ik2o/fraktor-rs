# stream-island-actors 実装計画 5.1〜5.3

## 対象タスク
- 5.1 materialized graph 単位の `KillSwitchStateHandle` 注入経路、または同等に単純な内部集約構造を追加する。
- 5.2 `Materialized::unique_kill_switch()` / `shared_kill_switch()` が複数 island graph でも graph 全体の lifecycle surface になることを test で固定する。
- 5.3 `Materialized` の multi-island 挙動が先頭 island の `StreamShared` だけに依存しないことを内部実装で保証する。

## 実装方針
- `ActorMaterializer` で materialized graph ごとに 1 つの `KillSwitchStateHandle` を生成し、全 island stream と `Materialized` に同じ handle を渡す。
- `Materialized::unique_kill_switch()` / `shared_kill_switch()` は先頭 `StreamShared` から導出せず、保持した graph-scoped state から生成する。
- `IslandSplitter` / `SingleIslandPlan` の変換で、元 graph に付いていた external shared kill switch state を island plan に引き継ぐ。
- 5.4 以降の actor command 伝播、terminal priority aggregation、diagnostic 追加には触れない。

## 検証
- `rtk cargo test -p fraktor-stream-core-rs materialized_unique_kill_switch_state_is_shared_by_all_islands`
- `rtk cargo test -p fraktor-stream-core-rs materialized_shared_kill_switch_state_is_shared_by_all_islands`
- `rtk cargo test -p fraktor-stream-core-rs external_shared_kill_switch_state_is_shared_by_all_islands`
- `rtk cargo test -p fraktor-stream-core-rs actor_materializer`
- `rtk rustup run nightly-2025-12-01 cargo fmt --all --check`
- `rtk git diff --check`

TAKT 実行中のため、`./scripts/ci-check.sh ai all` は final-ci ムーブメント以外では実行しない。
