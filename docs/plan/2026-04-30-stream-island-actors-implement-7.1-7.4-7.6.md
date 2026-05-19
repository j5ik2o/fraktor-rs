# stream-island-actors 実装計画 7.1 / 7.4 / 7.6

## 対象変更

- change: `stream-island-actors`
- tasks_path: `openspec/changes/stream-island-actors/tasks.md`

## 今回のバッチ

| タスクID | 内容 |
|----------|------|
| 7.1 | `Flow::add_attributes(Attributes::async_boundary())` が stage attribute ではなく graph attribute に留まる場合の扱いを確認し、必要なら別 task として切り出す |
| 7.4 | `async_with_dispatcher()` の rustdoc に downstream island dispatcher の意味を記載する |
| 7.6 | `Materialized::unique_kill_switch()` / `shared_kill_switch()` の rustdoc に、複数 island graph では graph 全体を代表することを記載する |

## 実装方針

- 7.1 は write_tests ステップで追加済みの回帰テストにより、`Flow::add_attributes(Attributes::async_boundary())` が graph attribute に留まり、island split を起こさないことを固定する。実装変更や別 task 切り出しは行わない。
- 7.4 は `Source::async_with_dispatcher()` と `Flow::async_with_dispatcher()` の rustdoc を揃え、dispatcher が async boundary の downstream island actor に適用されることを明記する。
- 7.6 は `Materialized::unique_kill_switch()` と `shared_kill_switch()` の rustdoc に、複数 island graph 全体の shutdown / abort / terminal lifecycle surface を代表することを明記する。
- 公開 API の追加や後方互換用コードは追加しない。
- `./scripts/ci-check.sh ai all` は final-ci ムーブメント以外では実行しない。

## 検証

- `rtk cargo test -p fraktor-stream-core-rs flow_add_attributes_async_boundary_stays_graph_attribute_and_does_not_split_island -- --nocapture`
- `rtk cargo test -p fraktor-stream-core-rs flow_async_with_dispatcher_assigns_dispatcher_to_downstream_island_actor -- --nocapture`
- `rtk cargo test -p fraktor-stream-core-rs materialized_unique_kill_switch_state_is_shared_by_all_islands -- --nocapture`
- `rtk cargo test -p fraktor-stream-core-rs materialized_shared_kill_switch_state_is_shared_by_all_islands -- --nocapture`
