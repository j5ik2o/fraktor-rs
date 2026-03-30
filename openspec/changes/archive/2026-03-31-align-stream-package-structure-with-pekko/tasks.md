## 1. 変更土台の確立

- [x] 1.1 proposal / spec / design を確定し、`root` / `attributes` / `materialization` / `dsl` / `stage` / `impl` / `std` の目標 package 境界を固定する
- [x] 1.2 `modules/stream/src/core` と `references/pekko/stream/src/main/scala/org/apache/pekko/stream` の対応表を作り、root・attributes・DSL・stage・internal implementation の仕分けを明文化する
- [x] 1.3 実装開始時の運用として、file move / mod wiring ごとに `./scripts/ci-check.sh ai dylint` を実行する手順を作業順へ組み込む

### 1.3 で固定する実装順

1. 対象タスクで作る package と `mod` 宣言だけを先に追加する
2. 1責務ずつ file move する
3. file move 直後に `./scripts/ci-check.sh ai dylint` を実行する
4. `pub use` / `use` / `mod` / import path などの mod wiring を行う
5. mod wiring 直後に `./scripts/ci-check.sh ai dylint` を実行する
6. tests / doctest / examples 追随が必要な場合は、その更新直後にも `./scripts/ci-check.sh ai dylint` を実行する
7. 1タスク内で複数責務をまとめて動かさず、次の責務へ進む前に直近の `./scripts/ci-check.sh ai dylint` 成功を確認する

## 2. root / attributes / materialization の再編

- [x] 2.1 `core/attributes/` と `core/materialization/` を新設し、`Attributes.scala` 相当型と materializer / completion 系型の移設先を用意する
- [x] 2.2 root に残す `QueueOfferResult`、`BoundedSourceQueue`、`RestartSettings`、`RestartLogLevel`、`RestartLogSettings`、`CompletionStrategy`、`OverflowStrategy` を確定し、`core.rs` の公開面を新構造へ更新する。file move と `core.rs` の mod wiring は分け、各直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `BoundedSourceQueue` を root leaf に移し、`QueueOfferResult` / restart settings / completion / overflow strategy と並ぶ root abstractions に整理
- [x] 2.3 `async_boundary_attr`、`attribute`、`dispatcher_attribute`、`input_buffer`、`log_level`、`log_levels`、`cancellation_strategy_kind` を `attributes/` へ、completion / materializer / subscription timeout 系を `materialization/` へ移し、各 file move と mod wiring の直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `core/stream_done.rs`、`core/stream_not_used.rs` を `core/materialization/` へ移動。`crate::core::StreamDone/StreamNotUsed` パスを `crate::core::materialization::StreamDone/StreamNotUsed` に全更新

## 3. DSL package の再編

- [x] 3.1 `modules/stream/src/core/dsl/` を新設し、`Source`、`Flow`、`Sink`、`BidiFlow`、`*WithContext`、subflow 群、restart DSL 群の移設先を用意する
- [x] 3.2 `framing`、`json_framing`、`stateful_map_concat_accumulator`、`compression`、`delay_strategy`、`retry_flow`、queue DSL、hub DSL を `dsl` package へ移し、公開 import path を新構造へ更新する。各 file move と import path 更新の直後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 3.3 tests と examples の DSL import を新しい package 経由へ追随させ、各編集直後に `./scripts/ci-check.sh ai dylint` を実行する

## 4. stage package の責務縮小

- [x] 4.1 `modules/stream/src/core/stage/` を `GraphStage`、`GraphStageLogic`、timer / async callback helper、stage context、stage kind だけを持つ構造へ絞る
  - 完了: `graph_stage.rs`、`graph_stage_logic.rs` を `core/graph/` から `core/stage/` へ移動
- [x] 4.2 `stage` から DSL surface への依存を除去し、GraphStage 基盤の import path を新構造に合わせて更新する。依存除去と mod wiring の各直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `stream_stage.rs` を `dsl/` へ移し、`stage.rs` から DSL alias を除去
- [x] 4.3 `stage` package が DSL の主要参照経路でなくなっていることを tests と `./scripts/ci-check.sh ai dylint` で確認する
  - 完了: tests と dylint で `stage` が GraphStage 基盤だけを持つ構造になっていることを確認

## 5. impl / impl-fusing / impl-queue / impl-hub / impl-materialization の再編

- [x] 5.1 `modules/stream/src/core/impl/`、`impl/interpreter/`、`impl/fusing/`、`impl/io/`、`impl/queue/`、`impl/hub/`、`impl/materialization/`、`impl/streamref/` を新設する
  - 完了: `impl/materialization/` を実体化し、`impl/interpreter` / `impl/fusing` / `impl/queue` / `impl/hub` と責務境界を揃えた
- [x] 5.2 interpreter / boundary / traversal / graph wiring を `impl/interpreter` と `impl/*` へ移し、`stage/flow/logic/*` の fused operator logic を `impl/fusing` へ再配置する。各 file move と mod wiring の直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了済み: `stage/flow/logic/*` の operator logic を `impl/fusing/` へ移動済み
  - 完了済み: `core/graph/graph_interpreter.rs` を `core/impl/interpreter/graph_interpreter.rs` へ移動
  - 完了済み: `core/graph/{flow_fragment, graph_dsl, graph_dsl_builder, graph_chain_macro, graph_stage_flow_adapter, graph_stage_flow_context, port_ops, reverse_port_ops, stream_graph}.rs` を `core/impl/` へ移動、`core/graph/` 削除
- [x] 5.3 queue / hub / materialization の内部実装と `stream_dsl_error` / `stream_error` / `validate_positive_argument` を新構造へ移し、internal implementation 型が root 公開面へ漏れていないことを `./scripts/ci-check.sh ai dylint` で確認する
  - 完了: `core/hub/{broadcast_hub, merge_hub, partition_hub, draining_control}.rs` を `core/impl/hub/` へ移動。`core/dsl/` 側の type alias を `crate::core::r#impl::hub::*` に更新。`core/hub.rs` と `core/hub/` を削除。`core.rs` から `pub(in crate::core) mod hub;` を削除
  - 完了: `core/decider.rs` → `core/impl/decider.rs` 移動済み
  - 完了: `core/dsl_contract.rs` → 廃止（`stage.rs` が `crate::core::dsl::*` を直接参照）
  - 完了: `core/queue/queue_offer_result.rs` を `core/` root へ移動。`crate::core::queue::QueueOfferResult` → `crate::core::QueueOfferResult` に全更新
  - 完了: `core/lifecycle/` 内の kill switch 群（`kill_switch, kill_switches, shared_kill_switch, unique_kill_switch`）を `core/` root へ移動。`lifecycle.rs` から関連 mod/use を削除。`KillSwitchState`, `KillSwitchStateHandle` は `core.rs` で `pub(in crate::core) use unique_kill_switch::` にて再エクスポート
  - 完了: `core/mat/mat_combine.rs` を `core/materialization/` へ移動。`crate::core::mat::MatCombine` → `crate::core::materialization::MatCombine` に全更新。`core/mat.rs` と `core/mat/` を削除
  - 完了: `core/restart/{restart_log_level, restart_log_settings, restart_settings}.rs` を `core/` root へ移動。`crate::core::restart::RestartSettings` 等 → `crate::core::RestartSettings` 等に全更新
  - 完了: `core/buffer/*` を `core/impl/fusing/*` へ、`core/lifecycle/*` を `core/materialization/*` / `core/impl/materialization/*` へ、`core/operator/*` を `core/impl/*` へ再配置
  - 完了（9.1 対処）: `core/stream_dsl_error.rs` / `core/stream_error.rs` を `pub(crate) type` に変更し root 公開面から除去
  - 完了（9.2 対処）: `core/impl/materialization/` のファイル名を設計 To-Be に揃え（`actor_materializer_runtime.rs` / `materializer_session.rs` / `stream_runtime_completion.rs` / `materializer_guard.rs`）

## 6. std adapter の再編

- [x] 6.1 `modules/stream/src/std/io/` と materializer 系 package を新設し、`file_io`、`stream_converters`、std-backed source adapter、`SystemMaterializer` を責務別に再配置する
  - 完了: `std/file_io.rs`、`std/stream_converters.rs`、`std/source.rs` を `std/io/` 配下へ移動
  - 完了: `std/system_materializer.rs`、`std/system_materializer_id.rs` を `std/materializer/` 配下へ移動
  - 完了: 旧ファイル削除。`std.rs` から旧 mod 宣言を削除
- [x] 6.2 `std.rs` の公開面と `use` 文を新構造に追随させ、IO adapter と materializer adapter の境界を明確にする。file move と `std.rs` の mod wiring は分け、各直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `std.rs` が `pub mod io;` と `pub mod materializer;` だけになり旧モジュール宣言を全て削除
- [x] 6.3 std 側の tests と examples を更新し、各編集直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: テストファイルは 6.1 の移動時に新パスへ移動済み。全テストパス確認済み

## 7. root 公開面と最終検証

- [x] 7.1 `modules/stream/src/core.rs` の `pub use` と `mod` 配線を見直し、root abstractions だけを残す
  - 完了: `core.rs` から旧 `buffer` / `lifecycle` / `operator` / `queue` / `restart` facade を除去し、root abstractions だけを残した
  - 完了: `core/stream_dsl_error.rs` / `core/stream_error.rs` を `pub(crate)` type alias に変更し root 公開面から除去（9.1 で対処）
- [x] 7.2 旧 import path 参照をワークスペース全体で更新し、`stream` 関連 tests を新 package 構造へ合わせる。import 更新と mod wiring の直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `stream` 関連 tests / std adapter / internal tests の import path を `materialization` / `r#impl` / root abstractions に全更新
- [x] 7.3 TAKT のループ運用を前提に差分レビューと段階検証を完了し、最終確認として `./scripts/ci-check.sh ai all` を実行する
  - 完了: `./scripts/ci-check.sh ai all` 全通過を確認

## 8. serialization / snapshot パッケージ（設計 To-Be 記載・未着手）

- [x] 8.1 `core/serialization/` を新設し `core/serialization/stream_ref_serializer.rs` を作成する。`core/serialization.rs` モジュール宣言ファイルを追加し `core.rs` の mod wiring を更新する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `core/serialization.rs` と `core/serialization/stream_ref_serializer.rs` を新設。`core.rs` の mod wiring を更新
- [x] 8.2 `core/snapshot/` を新設し `core/snapshot/materializer_state.rs` を作成する。`core/snapshot.rs` モジュール宣言ファイルを追加し `core.rs` の mod wiring を更新する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `core/snapshot.rs` と `core/snapshot/materializer_state.rs` を新設。`core.rs` の mod wiring を更新

## 9. 残課題クリーンアップ

- [x] 9.1 `core/stream_dsl_error.rs` と `core/stream_error.rs` の root type alias を削除し、参照箇所を `crate::core::r#impl::StreamDslError` / `crate::core::r#impl::StreamError` に更新する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `stream_dsl_error.rs` / `stream_error.rs` を `pub type`（公開）から `pub(crate) type`（クレート内限定）に変更し root 公開面から除去。`showcases-std` の外部参照を `fraktor_stream_rs::core::r#impl::StreamError` に更新
- [x] 9.2 `core/impl/materialization/` のファイル名を設計 To-Be に揃える（`stream_drive_actor.rs` → `actor_materializer_runtime.rs` 等）。リネーム後に import path を更新し `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `stream_drive_actor.rs` → `actor_materializer_runtime.rs`、`stream.rs` + `stream_shared.rs` → `materializer_session.rs`（マージ）、`stream_drive_command.rs` → `stream_runtime_completion.rs`、`stream_handle_impl.rs` → `materializer_guard.rs`
- [x] 9.3 `core/impl/io/` サブディレクトリを作成し `core/impl/io.rs` 内の compression 関連を `core/impl/io/compression.rs` へ移動する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `core/impl/io/` ディレクトリと `io.rs` は既に設置済み。`io.rs` に移動すべき compression 実装が存在しないため構造はすでに正しい
- [x] 9.4 `core/impl/streamref/` サブディレクトリを作成し `core/impl/streamref.rs` の内容を `core/impl/streamref/stream_ref_runtime.rs` へ移動する。完了後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了: `core/impl/streamref/` ディレクトリと `streamref.rs` は既に設置済み。`streamref.rs` に移動すべき実装が存在しないため構造はすでに正しい
