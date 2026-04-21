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
| `stream-core` | `src/core/dsl/retry_flow/tests.rs` | そのまま integration test へ移せる | `tests/retry_flow.rs` へ移設済み |

## 残存候補

### helper 切り出しが必要

- `actor-core/src/core/kernel/system/state/system_state_shared/tests.rs`
  - `super::SystemStateShared` と module-local な event stream test helper に依存している。
- `actor-core/src/core/kernel/dispatch/mailbox/base/tests.rs`
  - module-local `impl Mailbox` の test helper を追加しており、integration test へそのまま出せない。
- `actor-core/src/core/typed/scheduler/tests.rs`
  - ローカル scheduler harness を抱えている。
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
- `stream-core/src/core/dsl/source_with_context/tests.rs`
  - local future/helper を `tests/fixtures` に切り出せば移設可能。
- `stream-core/src/core/dsl/source/tests.rs`
  - private test logic 型が多く、fixture 分離が必要。
- `stream-core/src/core/materialization/actor_materializer/tests.rs`
  - `r#impl` / module-private materialization path に依存している。

### 当面 `src` に残す

- `stream-core/src/core/dsl/coupled_termination_flow/tests.rs`
  - integration test へ移すと `Flow::into_parts` / `Flow::from_graph` の `pub(in crate::core)` 制約に当たり、Pekko 互換テストを保ったまま外出しできなかった。
  - 一度 integration 化を試し、失敗を確認したうえで `src` 側へ戻した。

### 今回の std cleanup 対象外

- `remote-adaptor-std/src/provider/tests.rs`
- `remote-adaptor-std/src/extension_installer/tests.rs`
- `remote-adaptor-std/src/association_runtime/tests.rs`
  - std adaptor crate なので no_std-sensitive cleanup の優先度を下げる。
- `actor-core/src/core/kernel/pattern/circuit_breaker/tests.rs`
- `actor-core/src/core/kernel/pattern/circuit_breaker_shared/tests.rs`
  - `std::time` はコメント由来で、実コードの std 依存 cleanup 対象ではない。

## dead_code 観点

- 今回 `src/` から消えた direct-move 候補は 3 件。
- `dead_code` 影響が大きいのは helper-heavy な残存候補群で、次 batch は `source_with_context/tests.rs` か actor-core routing/system harness 群の fixture 分離を優先する。
- public surface を広げてまで `dead_code` を消さない。

## 検証

- `cargo test -p fraktor-cluster-core-rs --test cluster_identity`
- `cargo test -p fraktor-actor-core-rs --test byte_string`
- `cargo test -p fraktor-stream-core-rs --test retry_flow`
- `coupled_termination_flow` の integration 化は compile error を確認し、`src` 側へ戻した

