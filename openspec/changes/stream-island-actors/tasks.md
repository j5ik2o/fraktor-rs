## 1. 現状固定と退行テスト準備

- [ ] 1.1 `Source::async()` / `Flow::async()` が stage attribute として async boundary を付与していることを確認する。
- [ ] 1.2 `Source::async_with_dispatcher()` / `Flow::async_with_dispatcher()` が downstream island dispatcher candidate として解釈されることを `IslandSplitter` tests で固定する。
- [ ] 1.3 `SingleIslandPlan::dispatcher()` が materialization 前に取得できることを unit test で固定する。
- [ ] 1.4 現在の `StreamDriveActor` が複数 handle を直列 drive している箇所を洗い出し、置換対象を明確にする。
- [ ] 1.5 dispatcher 反映を観測するための test-only dispatcher factory または actor snapshot 経路を決める。

## 2. island actor 実行単位の追加

- [ ] 2.1 `StreamIslandActor` を追加し、1 actor が 1 `StreamHandleImpl` だけを所有するようにする。
- [ ] 2.2 `StreamIslandCommand` を追加し、`Drive` / `Cancel` / `Shutdown` を扱う。
- [ ] 2.3 `StreamIslandActor` の `Drive` command が自分の mailbox 内で `handle.drive()` を呼ぶことを unit test で固定する。
- [ ] 2.4 terminal state に到達した island actor がそれ以上 drive されないことを固定する。
- [ ] 2.5 `StreamDriveActor` を削除または tick fanout 専用へ縮小し、複数 handle を直接 drive する責務を残さない。

## 3. ActorMaterializer の materialization 変更

- [ ] 3.1 `ActorMaterializer` が `IslandSplitter::split(...)` の island ごとに `StreamIslandActor` を spawn するようにする。
- [ ] 3.2 `SingleIslandPlan::dispatcher()` を `into_stream_plan()` 前に読み取り、`Props::with_dispatcher_id(...)` に反映する。
- [ ] 3.3 dispatcher 指定がない island は default dispatcher を使うようにする。
- [ ] 3.4 未登録 dispatcher 指定時に materialization が失敗し、default dispatcher へフォールバックしないことを integration test で固定する。
- [ ] 3.5 island actor spawn 途中で失敗した場合、起動済み actor / tick resource / boundary resource を rollback する。

## 4. tick 供給と lifecycle 管理

- [ ] 4.1 tick 供給方式を island ごとの scheduler job にするか tick fanout actor にするか決定する。
- [ ] 4.2 どちらの方式でも `handle.drive()` が scheduler callback や fanout actor から直接呼ばれないことを実装で保証する。
- [ ] 4.3 materialized stream ごとの island actor 集合を追跡する内部構造を追加する。
- [ ] 4.4 `ActorMaterializer::shutdown()` が全 island actor と tick resource を停止するようにする。
- [ ] 4.5 shutdown failure を `StreamError` または actor error として観測できるようにし、戻り値を黙殺しない。

## 5. composite materialized handle

- [ ] 5.1 複数 island graph 全体を代表する composite handle または同等の内部構造を追加する。
- [ ] 5.2 `cancel()` が全 island actor へ cancel / shutdown を伝播することを integration test で固定する。
- [ ] 5.3 terminal state が graph 全体の状態として導出されることを test で固定する。
- [ ] 5.4 materializer snapshot または test-only diagnostic で island 数を観測できるようにする。
- [ ] 5.5 dispatcher id または actor id を test から検証できる経路を追加する。

## 6. boundary backpressure と terminal propagation

- [ ] 6.1 `IslandBoundaryShared` が actor 分離後の full / empty / completed / failed / cancelled state を表現できるか確認する。
- [ ] 6.2 表現できない場合、同じ contract を持つ boundary 型へ置き換える。
- [ ] 6.3 boundary full 時に upstream island が要素を保持して pending になることを test で固定する。
- [ ] 6.4 boundary empty かつ open 時に downstream island が pending になり、busy loop しないことを test で固定する。
- [ ] 6.5 upstream completion が pending 要素の後に downstream completion として観測されることを test で固定する。
- [ ] 6.6 upstream failure が downstream failure として観測されることを test で固定する。
- [ ] 6.7 downstream cancellation が upstream island へ伝播することを test で固定する。

## 7. DSL / showcase / 公開面の整合

- [ ] 7.1 `Flow::add_attributes(Attributes::async_boundary())` が stage attribute ではなく graph attribute に留まる場合の扱いを確認し、必要なら別 task として切り出す。
- [ ] 7.2 stream showcase は ActorSystem + ActorMaterializer + Sink 経由の実行だけを示すように維持する。
- [ ] 7.3 ActorSystem なしの直実行 API や `collect_values()` 相当 helper を公開 API に戻さない。
- [ ] 7.4 `async_with_dispatcher()` の rustdoc に downstream island dispatcher の意味を記載する。
- [ ] 7.5 カスタム stream mailbox selector は本 change に含めず、必要なら別 change として整理する。

## 8. 検証

- [ ] 8.1 `rtk cargo test -p fraktor-stream-core-rs` を実行する。
- [ ] 8.2 必要に応じて `rtk cargo test -p fraktor-showcases-std --features advanced` を実行する。
- [ ] 8.3 `rtk git diff --check` を実行する。
- [ ] 8.4 ソースコード編集後の最終確認として `rtk ./scripts/ci-check.sh ai all` を実行し、完了を待つ。
