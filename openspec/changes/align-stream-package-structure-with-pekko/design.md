## Context

`modules/stream/src/core` は現在、`stage` が DSL surface と GraphStage helper を同時に抱え、`graph` が interpreter / boundary / traversal / graph DSL を抱え、`stage/flow/logic` が operator 実装の集積所になっている。これは fraktor 内では動作しているが、Pekko 参照実装の `root`、`scaladsl` / `javadsl`、`stage`、`impl`、`impl/fusing`、`impl/io` という責務境界とは一致していない。

また `modules/stream/src/std` も `file_io`、`stream_converters`、`source`、`system_materializer` がフラットに並び、Pekko 側の IO / materializer 語彙との対応が弱い。今後 Pekko から operator や内部パターンを移植するたびに、参照先が `stage` なのか `graph` なのか `std` なのかを都度読み替える必要がある。

この変更では、Pekko の package を Rust にそのまま複製するのではなく、fraktor の `core` / `std` 分離を維持したまま、責務境界を Pekko に対応付けやすい形へ再編する。

設計基準は「Pekko 互換以上なら採用する」である。基本線は Pekko の責務境界に整合することとし、Pekko より曖昧になる独自化は採用しない。一方で、Rust 側でより高い凝集性、探索性、責務明確化を実現できる場合は、Pekko の package 名から意図的に外れても許容する。`shape/` package はその代表例であり、Pekko 非同型だが保守性では Pekko 互換以上と判断して採用する。

## As-Is / To-Be

### As-Is

現状の `modules/stream/src` は、DSL、GraphStage helper、interpreter、operator logic、std adapter が以下のように分散している。ここでは構造再編の対象になる production の directory と `.rs` を列挙する。

```text
modules/stream/src/
├── lib.rs
├── core.rs
├── std.rs
├── core/
│   ├── async_boundary_attr.rs
│   ├── attribute.rs
│   ├── attributes.rs
│   ├── buffer.rs
│   ├── completion.rs
│   ├── compression.rs
│   ├── decider.rs
│   ├── dispatcher_attribute.rs
│   ├── framing.rs
│   ├── graph.rs
│   ├── hub.rs
│   ├── io_result.rs
│   ├── json_framing.rs
│   ├── lifecycle.rs
│   ├── log_level.rs
│   ├── log_levels.rs
│   ├── mat.rs
│   ├── operator.rs
│   ├── queue.rs
│   ├── restart.rs
│   ├── shape.rs
│   ├── stage.rs
│   ├── stateful_map_concat_accumulator.rs
│   ├── stream_done.rs
│   ├── stream_dsl_error.rs
│   ├── stream_error.rs
│   ├── stream_not_used.rs
│   ├── subscription_timeout_mode.rs
│   ├── subscription_timeout_settings.rs
│   ├── substream_cancel_strategy.rs
│   ├── supervision_strategy.rs
│   ├── testing.rs
│   ├── throttle_mode.rs
│   ├── validate_positive_argument.rs
│   ├── buffer/
│   │   ├── cancellation_strategy_kind.rs
│   │   ├── completion_strategy.rs
│   │   ├── demand.rs
│   │   ├── demand_tracker.rs
│   │   ├── input_buffer.rs
│   │   ├── overflow_strategy.rs
│   │   ├── stream_buffer.rs
│   │   └── stream_buffer_config.rs
│   ├── graph/
│   │   ├── boundary_sink_logic.rs
│   │   ├── boundary_source_logic.rs
│   │   ├── flow_fragment.rs
│   │   ├── graph_chain_macro.rs
│   │   ├── graph_dsl.rs
│   │   ├── graph_dsl_builder.rs
│   │   ├── graph_interpreter.rs
│   │   ├── graph_stage.rs
│   │   ├── graph_stage_flow_adapter.rs
│   │   ├── graph_stage_flow_context.rs
│   │   ├── graph_stage_logic.rs
│   │   ├── island_boundary.rs
│   │   ├── island_splitter.rs
│   │   ├── port_ops.rs
│   │   ├── reverse_port_ops.rs
│   │   └── stream_graph.rs
│   ├── hub/
│   │   ├── broadcast_hub.rs
│   │   ├── draining_control.rs
│   │   ├── merge_hub.rs
│   │   └── partition_hub.rs
│   ├── lifecycle/
│   │   ├── drive_outcome.rs
│   │   ├── kill_switch.rs
│   │   ├── kill_switches.rs
│   │   ├── shared_kill_switch.rs
│   │   ├── stream.rs
│   │   ├── stream_drive_actor.rs
│   │   ├── stream_drive_command.rs
│   │   ├── stream_handle.rs
│   │   ├── stream_handle_id.rs
│   │   ├── stream_handle_impl.rs
│   │   ├── stream_shared.rs
│   │   ├── stream_state.rs
│   │   └── unique_kill_switch.rs
│   ├── mat/
│   │   ├── actor_materializer.rs
│   │   ├── actor_materializer_config.rs
│   │   ├── keep_both.rs
│   │   ├── keep_left.rs
│   │   ├── keep_none.rs
│   │   ├── keep_right.rs
│   │   ├── mat_combine.rs
│   │   ├── mat_combine_rule.rs
│   │   ├── materialized.rs
│   │   ├── materializer.rs
│   │   ├── materializer_lifecycle_state.rs
│   │   ├── materializer_snapshot.rs
│   │   ├── runnable_graph.rs
│   │   └── stream_completion.rs
│   ├── operator/
│   │   ├── default_operator_catalog.rs
│   │   ├── operator_catalog.rs
│   │   ├── operator_contract.rs
│   │   ├── operator_coverage.rs
│   │   └── operator_key.rs
│   ├── queue/
│   │   ├── actor_source_ref.rs
│   │   ├── bounded_source_queue.rs
│   │   ├── queue_offer_result.rs
│   │   ├── sink_queue.rs
│   │   ├── source_queue.rs
│   │   └── source_queue_with_complete.rs
│   ├── restart/
│   │   ├── delay_strategy.rs
│   │   ├── fixed_delay.rs
│   │   ├── linear_increasing_delay.rs
│   │   ├── restart_backoff.rs
│   │   ├── restart_log_level.rs
│   │   ├── restart_log_settings.rs
│   │   ├── restart_settings.rs
│   │   └── retry_flow.rs
│   ├── shape/
│   │   ├── bidi_shape.rs
│   │   ├── closed_shape.rs
│   │   ├── fan_in_shape2.rs
│   │   ├── fan_in_shape3.rs
│   │   ├── fan_in_shape4.rs
│   │   ├── fan_in_shape5.rs
│   │   ├── fan_in_shape6.rs
│   │   ├── fan_in_shape7.rs
│   │   ├── fan_in_shape8.rs
│   │   ├── fan_in_shape9.rs
│   │   ├── fan_in_shape10.rs
│   │   ├── fan_in_shape11.rs
│   │   ├── fan_in_shape12.rs
│   │   ├── fan_in_shape13.rs
│   │   ├── fan_in_shape14.rs
│   │   ├── fan_in_shape15.rs
│   │   ├── fan_in_shape16.rs
│   │   ├── fan_in_shape17.rs
│   │   ├── fan_in_shape18.rs
│   │   ├── fan_in_shape19.rs
│   │   ├── fan_in_shape20.rs
│   │   ├── fan_in_shape21.rs
│   │   ├── fan_in_shape22.rs
│   │   ├── fan_out_shape2.rs
│   │   ├── flow_shape.rs
│   │   ├── inlet.rs
│   │   ├── outlet.rs
│   │   ├── port_id.rs
│   │   ├── shape.rs
│   │   ├── sink_shape.rs
│   │   ├── source_shape.rs
│   │   ├── stream_shape.rs
│   │   ├── uniform_fan_in_shape.rs
│   │   └── uniform_fan_out_shape.rs
│   ├── stage/
│   │   ├── actor_sink.rs
│   │   ├── actor_source.rs
│   │   ├── async_callback.rs
│   │   ├── bidi_flow.rs
│   │   ├── flow.rs
│   │   ├── flow_group_by_sub_flow.rs
│   │   ├── flow_monitor.rs
│   │   ├── flow_monitor_impl.rs
│   │   ├── flow_monitor_state.rs
│   │   ├── flow_sub_flow.rs
│   │   ├── flow_with_context.rs
│   │   ├── restart_flow.rs
│   │   ├── restart_sink.rs
│   │   ├── restart_source.rs
│   │   ├── sink.rs
│   │   ├── source.rs
│   │   ├── source_group_by_sub_flow.rs
│   │   ├── source_sub_flow.rs
│   │   ├── source_with_context.rs
│   │   ├── stage_context.rs
│   │   ├── stage_kind.rs
│   │   ├── stream_stage.rs
│   │   ├── tail_source.rs
│   │   ├── timer_graph_stage_logic.rs
│   │   └── topic_pub_sub.rs
│   └── testing/
│       ├── stream_fuzz_runner.rs
│       ├── test_sink_probe.rs
│       └── test_source_probe.rs
└── std/
    ├── file_io.rs
    ├── source.rs
    ├── stream_converters.rs
    ├── system_materializer.rs
    └── system_materializer_id.rs
```

Pekko 側は次の責務境界を持つ。

```text
references/pekko/stream/src/main/scala/org/apache/pekko/stream/
├── root abstractions (*.scala)
├── scaladsl/
├── javadsl/
├── stage/
├── impl/
│   ├── fusing/
│   ├── io/
│   │   └── compression/
│   └── streamref/
├── serialization/
└── snapshot/
```

この差により、fraktor 側では以下の混在が起きている。

- `core/stage` が DSL surface と GraphStage primitive を同時に抱えている
- `core/graph` が公開寄りの graph DSL と internal interpreter を同時に抱えている
- `core/stage/flow/logic` が Pekko でいう `impl/fusing` 相当の責務を `stage` の内側に抱えている
- `std` 側の IO adapter と materializer extension が同じ階層に並んでいる

### To-Be

目標は、fraktor の `core` / `std` 分離を維持したまま、Pekko の責務境界へ対応付けやすい構造に再編することである。特に現行の `core/{mat,queue,buffer,hub}` は温存しない。各責務は `root` / `dsl` / `impl` のどこに属するかで再配置する。一方で shape 系は関連型どうしの凝集を優先し、`shape/` package にまとめて管理する。

```text
modules/stream/src/
├── lib.rs
├── core.rs
├── std.rs
├── core/
│   ├── attributes.rs
│   ├── bounded_source_queue.rs
│   ├── completion_strategy.rs
│   ├── io_result.rs
│   ├── overflow_strategy.rs
│   ├── queue_offer_result.rs
│   ├── restart_log_level.rs
│   ├── restart_log_settings.rs
│   ├── restart_settings.rs
│   ├── substream_cancel_strategy.rs
│   ├── supervision_strategy.rs
│   ├── throttle_mode.rs
│   ├── materialization.rs
│   ├── shape.rs
│   ├── dsl.rs
│   ├── stage.rs
│   ├── impl.rs
│   ├── serialization.rs
│   ├── snapshot.rs
│   ├── attributes/
│   │   ├── async_boundary_attr.rs
│   │   ├── attribute.rs
│   │   ├── cancellation_strategy_kind.rs
│   │   ├── dispatcher_attribute.rs
│   │   ├── input_buffer.rs
│   │   ├── log_level.rs
│   │   └── log_levels.rs
│   ├── materialization/
│   │   ├── completion.rs
│   │   ├── materializer.rs
│   │   ├── actor_materializer.rs
│   │   ├── actor_materializer_config.rs
│   │   ├── materialized.rs
│   │   ├── runnable_graph.rs
│   │   ├── mat_combine.rs
│   │   ├── mat_combine_rule.rs
│   │   ├── keep_both.rs
│   │   ├── keep_left.rs
│   │   ├── keep_none.rs
│   │   ├── keep_right.rs
│   │   ├── stream_completion.rs
│   │   ├── stream_done.rs
│   │   ├── stream_not_used.rs
│   │   ├── subscription_timeout_mode.rs
│   │   └── subscription_timeout_settings.rs
│   ├── dsl/
│   │   ├── compression.rs
│   │   ├── delay_strategy.rs
│   │   ├── framing.rs
│   │   ├── json_framing.rs
│   │   ├── retry_flow.rs
│   │   ├── stateful_map_concat_accumulator.rs
│   │   ├── source.rs
│   │   ├── sink.rs
│   │   ├── flow.rs
│   │   ├── bidi_flow.rs
│   │   ├── flow_with_context.rs
│   │   ├── source_with_context.rs
│   │   ├── flow_sub_flow.rs
│   │   ├── source_sub_flow.rs
│   │   ├── flow_group_by_sub_flow.rs
│   │   ├── source_group_by_sub_flow.rs
│   │   ├── tail_source.rs
│   │   ├── restart_flow.rs
│   │   ├── restart_sink.rs
│   │   ├── restart_source.rs
│   │   ├── queue.rs
│   │   ├── source_queue.rs
│   │   ├── source_queue_with_complete.rs
│   │   ├── sink_queue.rs
│   │   ├── hub.rs
│   │   ├── merge_hub.rs
│   │   ├── broadcast_hub.rs
│   │   ├── partition_hub.rs
│   │   └── draining_control.rs
│   ├── shape/
│   │   ├── inlet.rs
│   │   ├── outlet.rs
│   │   ├── shape.rs
│   │   ├── source_shape.rs
│   │   ├── sink_shape.rs
│   │   ├── flow_shape.rs
│   │   ├── bidi_shape.rs
│   │   ├── stream_shape.rs
│   │   ├── closed_shape.rs
│   │   ├── port_id.rs
│   │   ├── fan_in_shape2.rs
│   │   ├── fan_in_shape3.rs
│   │   ├── fan_in_shape4.rs
│   │   ├── fan_in_shape5.rs
│   │   ├── fan_in_shape6.rs
│   │   ├── fan_in_shape7.rs
│   │   ├── fan_in_shape8.rs
│   │   ├── fan_in_shape9.rs
│   │   ├── fan_in_shape10.rs
│   │   ├── fan_in_shape11.rs
│   │   ├── fan_in_shape12.rs
│   │   ├── fan_in_shape13.rs
│   │   ├── fan_in_shape14.rs
│   │   ├── fan_in_shape15.rs
│   │   ├── fan_in_shape16.rs
│   │   ├── fan_in_shape17.rs
│   │   ├── fan_in_shape18.rs
│   │   ├── fan_in_shape19.rs
│   │   ├── fan_in_shape20.rs
│   │   ├── fan_in_shape21.rs
│   │   ├── fan_in_shape22.rs
│   │   ├── fan_out_shape2.rs
│   │   ├── uniform_fan_in_shape.rs
│   │   └── uniform_fan_out_shape.rs
│   ├── stage/
│   │   ├── graph_stage.rs
│   │   ├── graph_stage_logic.rs
│   │   ├── timer_graph_stage_logic.rs
│   │   ├── async_callback.rs
│   │   ├── stage_context.rs
│   │   └── stage_kind.rs
│   ├── impl/
│   │   ├── graph.rs
│   │   ├── stream_graph.rs
│   │   ├── flow_fragment.rs
│   │   ├── graph_dsl.rs
│   │   ├── graph_dsl_builder.rs
│   │   ├── graph_chain_macro.rs
│   │   ├── graph_stage_flow_adapter.rs
│   │   ├── graph_stage_flow_context.rs
│   │   ├── port_ops.rs
│   │   ├── reverse_port_ops.rs
│   │   ├── interpreter.rs
│   │   ├── stream_dsl_error.rs
│   │   ├── stream_error.rs
│   │   ├── validate_positive_argument.rs
│   │   ├── interpreter/
│   │   │   ├── graph_interpreter.rs
│   │   │   ├── boundary_sink_logic.rs
│   │   │   ├── boundary_source_logic.rs
│   │   │   ├── island_boundary.rs
│   │   │   └── island_splitter.rs
│   │   ├── fusing/
│   │   │   ├── flow_logic.rs
│   │   │   ├── source_logic.rs
│   │   │   ├── sink_logic.rs
│   │   │   ├── buffer.rs
│   │   │   ├── demand.rs
│   │   │   ├── demand_tracker.rs
│   │   │   ├── stream_buffer.rs
│   │   │   └── stream_buffer_config.rs
│   │   ├── io/
│   │   │   └── compression.rs
│   │   ├── queue/
│   │   │   ├── actor_source_ref.rs
│   │   │   └── bounded_source_queue.rs
│   │   ├── hub/
│   │   │   ├── merge_hub.rs
│   │   │   ├── broadcast_hub.rs
│   │   │   └── partition_hub.rs
│   │   ├── materialization/
│   │   │   ├── actor_materializer_runtime.rs
│   │   │   ├── materializer_session.rs
│   │   │   ├── stream_runtime_completion.rs
│   │   │   └── materializer_guard.rs
│   │   └── streamref/
│   │       └── stream_ref_runtime.rs
│   ├── serialization/
│   │   └── stream_ref_serializer.rs
│   └── snapshot/
│       └── materializer_state.rs
└── std/
    ├── io.rs
    ├── materializer.rs
    ├── io/
    │   ├── file_io.rs
    │   ├── source.rs
    │   └── stream_converters.rs
    └── materializer/
        ├── system_materializer.rs
        └── system_materializer_id.rs
```

### 既存 `core/*` 再配置方針

| 現行 package | To-Be | ルール |
|--------------|-------|--------|
| `core/mat` | `core/materialization` + `core/impl/materialization/*` | 公開 contract は materialization、materializer 実装詳細は impl/materialization package に分離する |
| `core/shape` | `core/shape` | shape 系型は `shape/` package に集約し、関連抽象を同じ責務境界で管理する |
| `core/queue` | `core/dsl/queue` + `core/impl/queue` + root queue/result types | `SourceQueue` / `SinkQueue` 系 API は DSL、内部キュー実装は impl/queue、`QueueOfferResult` と `BoundedSourceQueue` は root に置く |
| `core/buffer` | root completion/overflow types + `core/attributes/input_buffer.rs` + `core/impl/fusing/*` | `core/buffer.rs` は残さず、`CompletionStrategy` と `OverflowStrategy` は root、`InputBuffer` は attributes、`DemandTracker` と `StreamBuffer` などの内部 buffer 実装は impl/fusing に置く |
| `core/hub` | `core/dsl/hub` + `core/impl/hub` | `MergeHub` / `BroadcastHub` / `PartitionHub` の利用 API は DSL、内部実装は impl に置く |
| `core/{async_boundary_attr,attribute,cancellation_strategy_kind,dispatcher_attribute,input_buffer,log_level,log_levels}` | `core/attributes/*` | Pekko では `Attributes.scala` の責務なので attributes package に集約する |
| `core/{framing,json_framing,stateful_map_concat_accumulator}` | `core/dsl/*` | Pekko では `scaladsl` / `javadsl` の DSL API として露出するため dsl に置く |
| `core/{compression,delay_strategy,retry_flow}` | `core/dsl/*` | 利用側 DSL から参照される stream API として扱い、impl/io や impl/fusing とは分離する |
| `core/{completion,stream_done,stream_not_used,subscription_timeout_mode,subscription_timeout_settings}` | `core/materialization/*` | completion と materialization lifecycle に属するため materialization に集約する |
| `core/{restart_settings,restart_log_level,restart_log_settings}` | root | Pekko の `RestartSettings.scala` と同様に root 側設定型として置く |
| `core/{stream_dsl_error,stream_error,validate_positive_argument}` | `core/impl/*` | fraktor 内部の補助・検証・失敗表現であり root の公開 API に置かない |

上記の通り、`shape` を除く現行 package はそのまま残さない。`shape` は例外として package にまとめ、関連型どうしの凝集を優先する。

### Pekko 対応方針

| Pekko 側 | fraktor 側 To-Be |
|----------|------------------|
| `org.apache.pekko.stream` root | `modules/stream/src/core` root abstractions + `attributes` + `materialization` + `shape` + root queue/result types + root restart settings + root completion/overflow types |
| `scaladsl` / `javadsl` | `modules/stream/src/core/dsl` |
| `stage` | `modules/stream/src/core/stage` |
| `impl` | `modules/stream/src/core/impl` |
| `impl/fusing` | `modules/stream/src/core/impl/fusing` |
| `impl/io` | `modules/stream/src/core/impl/io` と `modules/stream/src/std/io` |
| `impl/streamref` | `modules/stream/src/core/impl/streamref` |
| `scaladsl/Queue` / `javadsl/Queue` | `modules/stream/src/core/dsl/queue` |
| `scaladsl/Hub` / `javadsl/Hub` | `modules/stream/src/core/dsl/hub` |
| `impl/Buffers` / `impl/BoundedSourceQueue` | `modules/stream/src/core/impl/fusing/*` / `modules/stream/src/core/impl/queue` |
| `serialization` | `modules/stream/src/core/serialization` |
| `snapshot` | `modules/stream/src/core/snapshot` |

`javadsl` / `scaladsl` は Rust では二重化せず、`dsl` に一本化する。これは Pekko の package 名を厳密に複製するのではなく、Rust に必要な最小構造で責務境界を揃えるための意図的な差異である。したがって、Pekko の `javadsl` にある `Queue`、`Hub`、`Framing`、`JsonFraming`、`StatefulMapConcatAccumulator` も、このプロジェクトでは `core/dsl/*` へ寄せる。内部 operator 実装は `impl/fusing`、内部 I/O 実装は `impl/io` に分離する。

root に裸で置く型は、Pekko の `org.apache.pekko.stream` root に相当する抽象だけに絞る。`Attributes.scala` に属する概念、DSL API、internal helper は root に露出させない。

## Goals / Non-Goals

**Goals:**
- `modules/stream/src/core` の責務境界を Pekko の `root` / `dsl` / `stage` / `impl` / `impl/fusing` に対応付けやすい形へ整理する
- `Source`、`Flow`、`Sink` 系 DSL を `stage` から分離し、単一の Rust DSL package に集約する
- interpreter、boundary、flow logic を internal implementation package に集約し、DSL surface との混在を解消する
- 現行の `core/{mat,queue,buffer,hub}` を温存せず、`root` / `dsl` / `impl` の責務境界へ再配置する
- `core/shape` は shape 系抽象の凝集 package として維持する
- `modules/stream/src/std` の IO / materializer adapter を責務別 package に整理する
- import path、mod wiring、tests を新構造へ追随させ、`./scripts/ci-check.sh ai all` が通る状態まで定義する

**Non-Goals:**
- stream operator のセマンティクスや未実装機能をこの変更だけで追加すること
- Pekko の `javadsl` と `scaladsl` を Rust に二重実装すること
- `core` / `std` の層分離を崩して Pekko の directory をそのまま複写すること
- package 再編と無関係な runtime 挙動変更を同時に行うこと

## Decisions

### 1. Rust 向け DSL は `dsl` package へ一本化する
- 採用: Pekko の `scaladsl` / `javadsl` に相当する Rust 側の公開 DSL は、単一の `dsl` package に集約する
- 理由: Rust では Scala/Java の二重 DSL を持つ必要がなく、`Source`、`Flow`、`Sink` の参照経路は一つで十分だから
- 代替案: 既存の `stage` をそのまま DSL package と見なす
- 不採用理由: `stage` は Pekko では GraphStage 基盤の語彙であり、DSL surface を置くと責務境界がずれるため

### 2. `stage` は GraphStage 基盤だけを保持する
- 採用: `GraphStage`、`GraphStageLogic`、timer / async callback helper、stage context だけを `stage` に残す
- 理由: Pekko の `org.apache.pekko.stream.stage` と自然に対応付けられ、拡張 stage 実装の置き場が明確になるため
- 代替案: `stage` の中に DSL と helper を共存させる
- 不採用理由: DSL と GraphStage primitive が同居すると、公開 API と内部拡張 API の境界が読み取れないため

### 3. interpreter / operator logic は `impl` と `impl/fusing` に寄せる
- 採用: `graph` と `stage/flow/logic` に散在する runtime internals を `impl` と `impl/fusing` に再編する
- 理由: Pekko の `impl` / `impl/fusing` に対応する内部層を作ることで、operator 実装と DSL 公開面を分離できるため
- 代替案: `graph` と `stage/flow/logic` を現状維持し、命名だけ調整する
- 不採用理由: package path と責務境界が一致せず、Pekko 参照時のマッピングコストが残るため

### 4. std adapter は `io` と materializer 境界へ分ける
- 採用: `file_io`、`stream_converters`、std-backed source adapter は `std/io` 系 package、`SystemMaterializer` は materializer 系 package に寄せる
- 理由: I/O adapter と materializer lifecycle は責務が異なり、Pekko 側でも別の語彙で扱われているため
- 代替案: 現在の `std.rs` 配下のフラット構造を維持する
- 不採用理由: IO と materializer の責務が並列に見えず、拡張時の追加位置がぶれるため

### 5. `mat`、`queue`、`buffer`、`hub` は責務ごとに分解して再配置する
- 採用: `mat` は `materialization` と `impl/materialization`、`queue` は `dsl/queue` と `impl/queue` と root queue/result 型、`buffer` は root buffer 設定と `attributes/input_buffer` と `impl/fusing`、`hub` は `dsl/hub` と `impl/hub` に分解する
- 理由: Pekko でもこれらは root と DSL と impl にまたがっており、現行の一括 package のままだと対応関係が曖昧なまま残るため
- 代替案: 既存の `core/{mat,queue,buffer,hub}` を名前だけ残して内側だけ整理する
- 不採用理由: package 名が責務境界を誤誘導し続け、今回の再編目的を満たさないため

### 6. `shape` は package にまとめて維持する
- 採用: `Shape`、`Inlet`、`Outlet`、`SourceShape`、`SinkShape`、`FlowShape`、`BidiShape`、`FanInShape*`、`FanOutShape*` は `core/shape/` に集約する
- 理由: shape 系は相互参照が強く、1 つの責務境界でまとまっていた方がナビゲーションと保守性が高いため
- 補足: これは Pekko の root 直下配置からの意図的逸脱だが、Rust 側での探索性と型凝集を高めるための改善提案であり、「Pekko 互換以上」の原則に従って採用する
- 代替案: Pekko に厳密に合わせて root 直下へ展開する
- 不採用理由: Rust 側では型数が多く、root 直下へ拡散させると公開面とファイル探索の負荷が上がるため

### 7. root 公開面は root abstractions に絞り、互換 re-export は持たない
- 採用: `core.rs` は settings、strategies、shapes、materializer、queue/hub などの root abstractions に絞り、DSL / impl の互換 re-export は持たない
- 補足: `queue/hub` を root に丸ごと残す意味ではなく、root に置くべき基礎型だけを残す。shape は package にまとめつつ、必要な公開面だけを root から露出させる
- 理由: 正式リリース前であり、壊してでも package 境界を明確にする価値が高いため
- 代替案: 旧 path 互換の再 export を一定期間残す
- 不採用理由: package 再編の意図を弱め、`no-parent-reexport` 系 lint とも衝突しやすいため

## Risks / Trade-offs

- [Risk] import path の破壊的変更で tests/examples の修正量が増える → Mitigation: `dsl`、`stage`、`impl`、`std` の順で段階的に移し、各段階で `./scripts/ci-check.sh ai dylint` を実行する
- [Risk] `stage` と `impl` の切り分け途中で循環依存が発生する → Mitigation: 先に target package を作ってから file move し、bridge import を最小限に保つ
- [Risk] Pekko の `scaladsl` / `javadsl` を Rust の単一 `dsl` に畳む判断が曖昧さを生む → Mitigation: design と spec で「Rust は単一 DSL に一本化する」ことを明示し、二重 DSL を非目標に固定する
- [Risk] package 名だけ合わせて責務が変わらない中途半端な再編になる → Mitigation: `stage` から DSL を抜き、`impl/fusing` へ operator logic を集約することを完了条件に含める

## Migration Plan

1. proposal / spec / design を確定し、`dsl` / `stage` / `impl` / `std/io` / `std/materializer` の目標境界を固定する
2. `core` 側で target package を先に作り、`mod` 配線を用意する
3. DSL surface を `stage` から `dsl` へ移し、利用側 import を追随させる
4. GraphStage helper を `stage` に絞り、interpreter / operator logic を `impl` / `impl/fusing` へ移す
5. `std` を `io` / materializer 境界へ整理し、tests/examples を追随させる
6. 最終的に `./scripts/ci-check.sh ai all` を実行し、破綻がないことを確認する
