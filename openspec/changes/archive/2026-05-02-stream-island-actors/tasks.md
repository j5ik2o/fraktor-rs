> 前提: Pekko 互換仕様と Rust らしい設計の両立を、常に念頭に置いて判断する。

## 完了規律

- MUST: `stream-island-actors` は 1 つの change の中で、actor / mailbox 分離、dispatcher 反映、graph-wide lifecycle、downstream cancellation control plane、公開 API の絞り込みまで完結させる。
- MUST NOT: island split や boundary 基盤だけが入った段階で、この change を完了・archive 扱いにする。
- MUST NOT: 上記 core 項目を follow-up change へ送って、この change から外す。
- MAY: カスタム mailbox selector、remote / cluster stream、lock-free boundary 最適化のような拡張は後続 change へ送る。

## 1. 現状固定と退行テスト準備

- [x] 1.1 `Source::async()` / `Flow::async()` が stage attribute として async boundary を付与していることを既存 tests で固定する。
- [x] 1.2 `Source::async_with_dispatcher()` / `Flow::async_with_dispatcher()` が downstream island dispatcher candidate として解釈されることを `IslandSplitter` tests で固定する。
- [x] 1.3 `SingleIslandPlan::dispatcher()` が materialization 前に取得できることを unit test で固定する。
- [x] 1.4 現在の `StreamDriveActor` が複数 `StreamShared` を直列 drive している構造と、`ActorMaterializer` が multi-island graph を複数 `Stream` に materialize している構造を change 文書へ反映する。
- [x] 1.5 dispatcher 反映を観測するための経路を test-only actor snapshot / diagnostic に固定し、public snapshot API の拡張を core requirement から外す。

## 2. island actor 実行単位の追加

- [x] 2.1 `StreamIslandActor` を追加し、1 actor が 1 `StreamShared` だけを所有するようにする。
- [x] 2.2 `StreamIslandCommand` を追加し、`Drive` / `Cancel { cause: Option<StreamError> }` / `Shutdown` / `Abort(error)` を扱う。
- [x] 2.3 `StreamIslandActor` の `Drive` command が自分の mailbox 内で `stream.drive()` を呼ぶことを unit test で固定する。
- [x] 2.4 terminal state に到達した island actor がそれ以上 drive されないことを固定する。
- [x] 2.5 `StreamDriveActor` を削除し、複数 `StreamShared` を直接 drive する責務を残さない。

## 3. ActorMaterializer の materialization 変更

- [x] 3.1 `ActorMaterializer` が `IslandSplitter::split(...)` の island ごとに `StreamIslandActor` を spawn するようにする。
- [x] 3.2 `SingleIslandPlan::dispatcher()` を `into_stream_plan()` 前に読み取り、`Props::with_dispatcher_id(...)` に反映する。
- [x] 3.3 dispatcher 指定がない island は default dispatcher を使うようにする。
- [x] 3.4 未登録 dispatcher 指定時に materialization が失敗し、default dispatcher へフォールバックしないことを integration test で固定する。
- [x] 3.5 island actor spawn 途中で失敗した場合、起動済み actor / tick resource / boundary resource を rollback する。
- [x] 3.6 `ActorMaterializer::new_without_system` 相当の公開 helper を削除するか、`#[cfg(test)] pub(crate)` のテスト専用 API に縮小する。
- [x] 3.7 ActorSystem なしで `start()` / `materialize()` が成功する経路が残っていないことを test または compile check で固定する。

## 4. tick 供給と lifecycle 管理

- [x] 4.1 各 island actor に専用 scheduler job を持たせ、scheduler が対象 actor にのみ `Drive` を送るようにする。
- [x] 4.2 `stream.drive()` が scheduler callback から直接呼ばれず、必ず island actor の mailbox 内で実行されることを実装で保証する。
- [x] 4.3 materialized stream ごとの island actor 集合と scheduler handle 集合を追跡する内部構造を追加する。
- [x] 4.4 `ActorMaterializer::shutdown()` が全 island actor と tick resource を停止するようにする。
- [x] 4.5 shutdown failure を `StreamError` または actor error として観測できるようにし、戻り値を黙殺しない。
- [x] 4.6 `Drive` が coalescing され、1 island actor に未処理 `Drive` が複数積み上がらないことを unit / integration test で固定する。

## 5. graph-scoped kill switch / materialized lifecycle

- [x] 5.1 materialized graph 単位の `KillSwitchStateHandle` 注入経路、または同等に単純な内部集約構造を追加する。
- [x] 5.2 `Materialized::unique_kill_switch()` / `shared_kill_switch()` が複数 island graph でも graph 全体の lifecycle surface になることを test で固定する。
- [x] 5.3 `Materialized` の multi-island 挙動が先頭 island の `StreamShared` だけに依存しないことを内部実装で保証する。
- [x] 5.4 kill switch の `shutdown()` / `abort()` が全 island actor へ伝播することを integration test で固定する。
- [x] 5.5 terminal state の優先度（abort > failure > shutdown/cancel > completion）で graph 全体の状態が導出されることを test で固定する。
- [x] 5.6 `MaterializerState::stream_snapshots()` で island 数を観測できることを既存 tests で固定する。
- [x] 5.7 test-only actor snapshot / diagnostic で dispatcher id または actor id を検証できる経路を追加する。

## 6. boundary backpressure と terminal propagation

- [x] 6.1 `IslandBoundaryShared` と `BoundarySinkLogic` / `BoundarySourceLogic` が full / empty / completed / failed の基礎契約を unit test で固定する。
- [x] 6.2 actor 分離後も cancellation を表現できるか確認し、不足する場合は同じ contract を持つ boundary 型または補助 state を追加する。
- [x] 6.3 downstream cancellation を upstream island actor へ送る control plane を `MaterializedStreamGraph` または同等の構造に追加し、`BoundarySourceLogic::on_cancel()` を sole propagation path から外す。
- [x] 6.4 boundary full 時に upstream island が要素を保持して pending になることを test で固定する。
- [x] 6.5 boundary empty かつ open 時に downstream 側が `WouldBlock` で pending になることを unit test で固定する。
- [x] 6.6 actor 分離後も downstream island が busy loop しないことを regression test で固定する。
- [x] 6.7 upstream completion が pending 要素の後に downstream completion として観測されることを test で固定する。
- [x] 6.8 upstream failure が downstream failure として観測されることを test で固定する。
- [x] 6.9 downstream cancellation が boundary state だけでなく upstream island actor への `Cancel(cause)` command として伝播することを test で固定する。
- [x] 6.10 `IslandBoundaryShared` が actor 越境の並行アクセス下でも要素ロス・二重配送・不整合 terminal state を起こさないことを compile-time / stress test で固定する。
- [x] 6.11 `cancel` / `shutdown` / `abort` の 3 経路について、in-flight 要素の扱い（drain / discard / failure priority）を matrix test で固定する。

## 7. DSL / showcase / 公開面の整合

- [x] 7.1 `Flow::add_attributes(Attributes::async_boundary())` が stage attribute ではなく graph attribute に留まる場合の扱いを確認し、必要なら別 task として切り出す。
- [x] 7.2 stream showcase は ActorSystem + ActorMaterializer + Sink 経由の実行だけを示すように維持する。
- [x] 7.3 ActorSystem なしの直実行 API や `collect_values()` 相当 helper を公開 API に戻さない。
- [x] 7.4 `async_with_dispatcher()` の rustdoc に downstream island dispatcher の意味を記載する。
- [x] 7.5 カスタム stream mailbox selector は本 change に含めず、必要なら別 change として整理する。
- [x] 7.6 `Materialized::unique_kill_switch()` / `shared_kill_switch()` の rustdoc に、複数 island graph では graph 全体を代表することを記載する。

## 8. 検証

- [x] 8.1 fast feedback として `rtk cargo test -p fraktor-stream-core-rs` を実行する。
- [x] 8.2 必要に応じて `rtk cargo test -p fraktor-showcases-std --features advanced` を実行する。
- [x] 8.3 `rtk git diff --check` を実行する。
- [x] 8.4 最終 gate として `rtk ./scripts/ci-check.sh ai all` を実行し、完了を待つ。
- [x] 8.5 core completion gate（2, 3, 4, 5, 6, 7 のうち core capability に関わる項目）がすべて満たされていることを確認する。
- [x] 8.6 core capability の未達が 1 つでも残る場合、この change を完了・archive しないことを確認する。

> 前提: Pekko 互換仕様と Rust らしい設計の両立を、常に念頭に置いて判断する。
