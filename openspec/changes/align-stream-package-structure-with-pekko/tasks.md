## 1. 変更土台の確立

- [ ] 1.1 proposal / spec / design を確定し、`root` / `attributes` / `materialization` / `dsl` / `stage` / `impl` / `std` の目標 package 境界を固定する
- [ ] 1.2 `modules/stream/src/core` と `references/pekko/stream/src/main/scala/org/apache/pekko/stream` の対応表を作り、root・attributes・DSL・stage・internal implementation の仕分けを明文化する
- [ ] 1.3 実装開始時の運用として、file move / mod wiring ごとに `./scripts/ci-check.sh ai dylint` を実行する手順を作業順へ組み込む

## 2. root / attributes / materialization の再編

- [ ] 2.1 `core/attributes/` と `core/materialization/` を新設し、`Attributes.scala` 相当型と materializer / completion 系型の移設先を用意する
- [ ] 2.2 root に残す `QueueOfferResult`、`BoundedSourceQueue`、`RestartSettings`、`CompletionStrategy`、`OverflowStrategy` を確定し、`core.rs` の公開面を新構造へ更新する
- [ ] 2.3 `async_boundary_attr`、`attribute`、`dispatcher_attribute`、`input_buffer`、`log_level`、`log_levels` を `attributes/` へ、completion / materializer / subscription timeout 系を `materialization/` へ移し、各編集後に `./scripts/ci-check.sh ai dylint` を実行する

## 3. DSL package の再編

- [ ] 3.1 `modules/stream/src/core/dsl/` を新設し、`Source`、`Flow`、`Sink`、`BidiFlow`、`*WithContext`、subflow 群、restart DSL 群の移設先を用意する
- [ ] 3.2 `framing`、`json_framing`、`stateful_map_concat_accumulator`、queue DSL、hub DSL を `dsl` package へ移し、公開 import path を新構造へ更新する
- [ ] 3.3 tests と examples の DSL import を新しい package 経由へ追随させ、各編集後に `./scripts/ci-check.sh ai dylint` を実行する

## 4. stage package の責務縮小

- [ ] 4.1 `modules/stream/src/core/stage/` を `GraphStage`、`GraphStageLogic`、timer / async callback helper、stage context、stage kind だけを持つ構造へ絞る
- [ ] 4.2 `stage` から DSL surface への依存を除去し、GraphStage 基盤の import path を新構造に合わせて更新する
- [ ] 4.3 `stage` package が DSL の主要参照経路でなくなっていることを tests と dylint で確認する

## 5. impl / impl-fusing / impl-queue / impl-hub / impl-materialization の再編

- [ ] 5.1 `modules/stream/src/core/impl/`、`impl/interpreter/`、`impl/fusing/`、`impl/io/`、`impl/queue/`、`impl/hub/`、`impl/materialization/`、`impl/streamref/` を新設する
- [ ] 5.2 interpreter / boundary / traversal / graph wiring を `impl/interpreter` と `impl/*` へ移し、`stage/flow/logic/*` の fused operator logic を `impl/fusing` へ再配置する
- [ ] 5.3 queue / hub / materialization の内部実装と `stream_dsl_error` / `stream_error` / `validate_positive_argument` を新構造へ移し、internal implementation 型が root 公開面へ漏れていないことを確認する

## 6. std adapter の再編

- [ ] 6.1 `modules/stream/src/std/io/` と materializer 系 package を新設し、`file_io`、`stream_converters`、std-backed source adapter、`SystemMaterializer` を責務別に再配置する
- [ ] 6.2 `std.rs` の公開面と `use` 文を新構造に追随させ、IO adapter と materializer adapter の境界を明確にする
- [ ] 6.3 std 側の tests と examples を更新し、各編集後に `./scripts/ci-check.sh ai dylint` を実行する

## 7. root 公開面と最終検証

- [ ] 7.1 `modules/stream/src/core.rs` の `pub use` と `mod` 配線を見直し、root abstractions だけを残す
- [ ] 7.2 旧 import path 参照をワークスペース全体で更新し、`stream` 関連 tests を新 package 構造へ合わせる
- [ ] 7.3 TAKT のループ運用を前提に差分レビューと段階検証を完了し、最終確認として `./scripts/ci-check.sh ai all` を実行する
