# repo-wide-src-test-cleanup 棚卸しメモ

## 方針

- Pekko 互換を崩さない。テストのアサーションと対象 runtime behavior は変更しない。
- production 公開面を広げず、`src/` から切り離せる test だけを `tests/` へ移す。
- module-private API や test harness への依存が強いものは、無理に移さず backlog に残す。

## 完了した batch

| crate | 元ファイル | 区分 | 対応 |
|---|---|---|---|
| `cluster-core` | `src/core/identity/cluster_identity/tests.rs` | そのまま integration test へ移せる | `tests/cluster_identity.rs` へ移設済み |
| `actor-core` | `src/core/kernel/util/byte_string/tests.rs` | そのまま integration test へ移せる | `tests/byte_string.rs` へ移設済み |
| `actor-core` | `src/core/typed/scheduler/tests.rs` | public system API へ書き換えれば移せる | `tests/typed_scheduler.rs` へ移設済み |
| `stream-core` | `src/core/dsl/retry_flow/tests.rs` | そのまま integration test へ移せる | `tests/retry_flow.rs` へ移設済み |
| `stream-core` | `src/core/dsl/source_with_context/tests.rs` | private mat 観測を public graph 観測へ置き換えれば移せる | `tests/source_with_context.rs` へ移設済み |
| `stream-core` | `src/core/dsl/coupled_termination_flow/tests.rs` | private graph 操作を public `RunnableGraph` materialize へ置き換えれば移せる | `tests/coupled_termination_flow.rs` へ移設済み |

## 残存候補

### helper 切り出しが必要

- `actor-core/src/core/kernel/system/state/system_state_shared/tests.rs`
  - `super::SystemStateShared` と module-local な event stream test helper に依存している。
- `actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs`
  - module-local `impl Mailbox` の test helper を追加しており、integration test へそのまま出せない。
- `actor-core/src/core/kernel/actor/supervision/backoff_supervisor/tests.rs`
  - `register_cell` / strategy helper などの test harness が大きい。
- `actor-core/src/core/typed/delivery/tests.rs`
  - `test_system()` と `TestTickDriver` 前提の harness を抱えている。
- `actor-core/src/core/typed/dsl/behaviors/tests.rs`
  - tracing subscriber と大きな typed harness に依存している。
- `actor-core/src/core/typed/dsl/routing/scatter_gather_first_completed_router_builder/tests.rs`
  - typed system harness と builder failure injection を使っている。
- `actor-core/src/core/typed/dsl/routing/tail_chopping_router_builder/tests.rs`
  - typed system harness を使っている。
- `stream-core/src/core/dsl/source/tests.rs`
  - private test logic 型が多く、fixture 分離が必要。
- `stream-core/src/core/materialization/actor_materializer/tests.rs`
  - `r#impl` / module-private materialization path に依存している。

### 今回の std cleanup 対象外

- `remote-adaptor-std/src/provider/tests.rs`
- `remote-adaptor-std/src/extension_installer/tests.rs`
- `remote-adaptor-std/src/association_runtime/tests.rs`
  - std adaptor crate なので no_std-sensitive cleanup の優先度を下げる。
- `actor-core/src/core/kernel/pattern/circuit_breaker/tests.rs`
- `actor-core/src/core/kernel/pattern/circuit_breaker_shared/tests.rs`
  - `std::time` はコメント由来で、実コードの std 依存 cleanup 対象ではない。

## dead_code 観点

- 今回 `src/` から消えた / `tests/` へ移せた候補は 6 件。
- `dead_code` 影響が大きいのは helper-heavy な残存候補群で、次 batch は actor-core routing/system harness 群か `stream-core/src/core/dsl/source/tests.rs` の fixture 分離を優先する。
- public surface を広げてまで `dead_code` を消さない。

## 検証

- `cargo test -p fraktor-cluster-core-rs --test cluster_identity`
- `cargo test -p fraktor-actor-core-rs --test byte_string`
- `cargo test -p fraktor-actor-core-rs --features test-support --test typed_scheduler`
- `cargo test -p fraktor-stream-core-rs --test retry_flow`
- `cargo test -p fraktor-stream-core-rs --test source_with_context`
- `cargo test -p fraktor-stream-core-rs --test coupled_termination_flow`
