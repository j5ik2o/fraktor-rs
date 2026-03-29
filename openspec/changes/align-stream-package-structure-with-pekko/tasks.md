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
- [ ] 2.3 `async_boundary_attr`、`attribute`、`dispatcher_attribute`、`input_buffer`、`log_level`、`log_levels`、`cancellation_strategy_kind` を `attributes/` へ、completion / materializer / subscription timeout 系を `materialization/` へ移し、各 file move と mod wiring の直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 未完了: `core/stream_done.rs`、`core/stream_not_used.rs` が `core/materialization/` へ未移動（`core/` root に残存）

## 3. DSL package の再編

- [x] 3.1 `modules/stream/src/core/dsl/` を新設し、`Source`、`Flow`、`Sink`、`BidiFlow`、`*WithContext`、subflow 群、restart DSL 群の移設先を用意する
- [x] 3.2 `framing`、`json_framing`、`stateful_map_concat_accumulator`、`compression`、`delay_strategy`、`retry_flow`、queue DSL、hub DSL を `dsl` package へ移し、公開 import path を新構造へ更新する。各 file move と import path 更新の直後に `./scripts/ci-check.sh ai dylint` を実行する
- [x] 3.3 tests と examples の DSL import を新しい package 経由へ追随させ、各編集直後に `./scripts/ci-check.sh ai dylint` を実行する

## 4. stage package の責務縮小

- [ ] 4.1 `modules/stream/src/core/stage/` を `GraphStage`、`GraphStageLogic`、timer / async callback helper、stage context、stage kind だけを持つ構造へ絞る
  - 未完了: `graph_stage.rs`、`graph_stage_logic.rs` が `core/graph/` に残存。`core/stage/` への移動が未実施
- [ ] 4.2 `stage` から DSL surface への依存を除去し、GraphStage 基盤の import path を新構造に合わせて更新する。依存除去と mod wiring の各直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 未完了: 4.1 が未完了のため未着手
- [ ] 4.3 `stage` package が DSL の主要参照経路でなくなっていることを tests と `./scripts/ci-check.sh ai dylint` で確認する
  - 未完了: 4.1、4.2 が未完了のため未着手

## 5. impl / impl-fusing / impl-queue / impl-hub / impl-materialization の再編

- [x] 5.1 `modules/stream/src/core/impl/`、`impl/interpreter/`、`impl/fusing/`、`impl/io/`、`impl/queue/`、`impl/hub/`、`impl/materialization/`、`impl/streamref/` を新設する
  - 注意: `impl/hub/`、`impl/queue/`、`impl/io/`、`impl/materialization/`、`impl/streamref/` は空スタブ（`mod.rs` のみ）の状態
- [ ] 5.2 interpreter / boundary / traversal / graph wiring を `impl/interpreter` と `impl/*` へ移し、`stage/flow/logic/*` の fused operator logic を `impl/fusing` へ再配置する。各 file move と mod wiring の直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 完了済み: `stage/flow/logic/*` の operator logic を `impl/fusing/` へ移動済み
  - 未完了: `core/graph/graph_interpreter.rs` が `core/impl/interpreter/` へ未移動
  - 未完了: `core/graph/{flow_fragment, graph_dsl, graph_dsl_builder, graph_chain_macro, graph_stage_flow_adapter, graph_stage_flow_context, port_ops, reverse_port_ops, stream_graph}.rs` が `core/impl/` へ未移動（`core/graph/` に残存）
- [ ] 5.3 queue / hub / materialization の内部実装と `stream_dsl_error` / `stream_error` / `validate_positive_argument` を新構造へ移し、internal implementation 型が root 公開面へ漏れていないことを `./scripts/ci-check.sh ai dylint` で確認する
  - 未完了: `core/stream_dsl_error.rs`、`core/stream_error.rs`、`core/validate_positive_argument.rs` が `core/impl/` へ未移動
  - 未完了: `core/impl/hub/`、`core/impl/queue/`、`core/impl/materialization/`、`core/impl/io/`、`core/impl/streamref/` が空スタブのまま（実装未配置）

## 6. std adapter の再編

- [ ] 6.1 `modules/stream/src/std/io/` と materializer 系 package を新設し、`file_io`、`stream_converters`、std-backed source adapter、`SystemMaterializer` を責務別に再配置する
  - 部分完了: `std/io/` と `std/materializer/` ディレクトリは新設済み
  - 未完了: `std/io/file_io.rs`、`std/io/stream_converters.rs`、`std/materializer/system_materializer.rs` が実装を持たず、旧ファイルへの型エイリアス再エクスポートのみ
  - 未完了: 旧ファイル `std/file_io.rs`、`std/source.rs`、`std/stream_converters.rs`、`std/system_materializer.rs`、`std/system_materializer_id.rs` が `std.rs` から依然として宣言されており、削除未実施
- [ ] 6.2 `std.rs` の公開面と `use` 文を新構造に追随させ、IO adapter と materializer adapter の境界を明確にする。file move と `std.rs` の mod wiring は分け、各直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 未完了: `std.rs` が旧モジュール（`mod file_io`、`mod source`、`mod stream_converters`、`mod system_materializer`、`mod system_materializer_id`）を依然として宣言中
- [ ] 6.3 std 側の tests と examples を更新し、各編集直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 未完了: 6.1、6.2 が未完了のため未着手

## 7. root 公開面と最終検証

- [ ] 7.1 `modules/stream/src/core.rs` の `pub use` と `mod` 配線を見直し、root abstractions だけを残す
  - 未完了: `core.rs` が `pub mod graph`、`pub mod buffer`、`pub mod hub`、`pub mod lifecycle`、`mod mat`、`pub mod operator`、`pub mod queue`、`pub mod restart` 等、to-be に存在しないモジュールを依然として宣言中
- [ ] 7.2 旧 import path 参照をワークスペース全体で更新し、`stream` 関連 tests を新 package 構造へ合わせる。import 更新と mod wiring の直後に `./scripts/ci-check.sh ai dylint` を実行する
  - 未完了: 7.1 が未完了のため未着手
- [ ] 7.3 TAKT のループ運用を前提に差分レビューと段階検証を完了し、最終確認として `./scripts/ci-check.sh ai all` を実行する
  - 未完了: 構造移行が未完了
