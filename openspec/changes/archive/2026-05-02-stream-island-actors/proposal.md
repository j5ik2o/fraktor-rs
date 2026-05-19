> 前提: Pekko 互換仕様と Rust らしい設計の両立を、常に念頭に置いて判断する。

## Why

Pekko の stream `async()` / `async(dispatcher)` は、fused graph を island に分け、island ごとに独立した actor / mailbox / dispatcher で実行するための境界である。一方、現在の fraktor-rs は、最新コードですでに次の基盤までは持っている。

- `Source::async()` / `Flow::async()` と `async_with_dispatcher()` は stage attribute を付与する。
- `IslandSplitter` は graph を island に分割し、`SingleIslandPlan::dispatcher()` で downstream island の dispatcher candidate を保持する。
- `ActorMaterializer` は複数 island graph を island ごとの `Stream` / `StreamShared` に materialize し、`IslandBoundaryShared` で接続する。
- `BoundarySinkLogic` / `BoundarySourceLogic` は full / empty / completion / failure の基礎契約を unit test で固定済みである。

ただし runtime はまだ Pekko 互換の island actor 実行には届いていない。materialization 後の複数 island `StreamShared` は 1 つの `StreamDriveActor` に登録され、同一 actor の mailbox で直列に `drive()` される。また、`Materialized` が返す `unique_kill_switch()` / `shared_kill_switch()` は内部的に先頭 island の `StreamShared` へ依存しており、graph 全体の lifecycle surface になっていない。`ActorMaterializer::new_without_system` もまだ公開 helper のままである。

この状態では `async_with_dispatcher()` が dispatcher 属性を付けても実行時 dispatcher の選択に反映されず、Pekko 互換サンプルやテストが「ActorSystem 上で動いているが island は分離されるが、実行単位は分離されていない」状態になる。正式リリース前に、stream island を actor runtime に接続し、async boundary の意味論を Pekko に寄せつつ、公開 API は Rust らしく小さく保つ。

ここでいう「Pekko 互換」は、クラス名や内部構造の 1:1 再現ではなく、次の意味論が一致することを指す。

- island ごとの actor-owned execution
- actor mailbox を経由した boundary event / resume
- cancellation cause を伴う downstream cancel 伝播
- graph-wide lifecycle と snapshot 診断

## What Changes

### 0. この capability は 1 つの change で完結させる

`stream-island-actors` は、Pekko 互換の island runtime capability を 1 つの change として完成させる。ここでいう completed とは、単に island split と boundary が存在することではなく、少なくとも次が同時に成立した状態を指す。

- island ごとに独立した actor / mailbox で実行される
- `async_with_dispatcher()` が downstream island actor の dispatcher 選択に反映される
- kill switch / materializer shutdown / terminal observation が graph 全体を代表する
- downstream cancellation が upstream island actor へ control plane として伝播する
- ActorSystem なし materializer helper が公開 runtime API から退いている

したがって、この change は core capability の一部だけを実装した状態で完了扱いしてはならない（MUST NOT）。また、上記の core 項目を follow-up change へ送ってこの change を閉じてはならない（MUST NOT）。一方で、Pekko 互換性の成立に不要な拡張は後続 change へ送ってよい（MAY）。

### 1. 既存の island `Stream` を actor 実行単位へ昇格させる

`ActorMaterializer` は、`IslandSplitter` が生成した island ごとの `Stream` と island crossing を、`StreamDriveActor` へまとめて登録する代わりに、独立した island actor と crossing boundary へ割り当てる。各 actor は 1 つの island の `Stream` / `StreamShared` だけを所有し、自分の mailbox 上で drive / cancel / shutdown / abort command を処理する。

単一 island の stream も fused interpreter を 1 つの island actor 上で実行し、複数 island と同じ runtime path を通す。ただし `async()` により複数 island へ分かれた graph は、1 つの drive actor に全 island `StreamShared` を登録するのではなく、island ごとの actor 群として materialize する。

### 2. dispatcher 属性を island actor 起動へ反映する

`Source::async_with_dispatcher()` / `Flow::async_with_dispatcher()` が付与した dispatcher は、最新コードですでに `SingleIslandPlan::dispatcher()` として保持される。今回の change では、その値を materialization 時の island actor `Props` / dispatcher selector へ反映しなければならない。

dispatcher 指定がない island は actor system の default dispatcher を使う。stream 専用 mailbox 属性は現状存在しないため、本 change では「island ごとに actor mailbox が独立する」ことを対象にし、カスタム stream mailbox selector の追加は別 change とする。

### 3. 既存 boundary 契約を actor 分離後も維持し、不足している cancellation 制御を補う

既存の `IslandBoundaryShared`、`BoundarySinkLogic`、`BoundarySourceLogic` は、full / empty / completion / failure の基礎契約をすでに実装している。今回の change では、その契約を actor 分離後も壊さずに維持しつつ、現在は `BoundarySourceLogic::on_cancel()` が boundary 完了へ寄せている downstream cancellation を、明示的な control plane 付きの graph-wide stop request に引き上げる。

- boundary full は upstream island の局所 backpressure として扱う。
- boundary empty は downstream island の pending 状態として扱い、busy loop しない。
- upstream failure / completion は downstream island へ伝播する。
- downstream cancel は `Cancel { cause: Option<StreamError> }` として upstream island へ伝播する。
- graph-wide graceful stop は `Shutdown` に限定する。
- graph-wide failure stop は `Abort(error)` に限定する。

### 4. materialized lifecycle は graph 単位の kill switch / diagnostics で表す

複数 island の materialization では、公開 surface は `Materialized::unique_kill_switch()` / `Materialized::shared_kill_switch()` と diagnostics のままにし、その意味だけを graph 全体へ拡張する。Pekko 互換の lifecycle を達成するために新しい公開 handle API を足すのではなく、まずは graph 単位の `KillSwitchStateHandle` 共有、または最小限の内部集約構造で表現する。

公開面では、先頭 island の `StreamShared` から kill switch を導出する現在の実装を、そのまま複数 island graph の意味論として扱ってはならない。`MaterializerState::stream_snapshots()` は island ごとの診断経路として維持してよいが、shutdown / abort / terminal observation は materialized graph 全体に対して定義する。

`ActorMaterializer::shutdown()` は、materializer が起動した全 island actor と tick / drive resource を決定的に停止する。停止失敗は握りつぶさず、観測可能な `StreamError` または actor error として扱う。

### 5. tests と showcases を Pekko 互換の意味論へ寄せる

`async()` / `async_with_dispatcher()` のテストは、値が通るだけでなく、island 数、dispatcher 反映、boundary backpressure、kill switch / shutdown 伝播を検証する。showcase では ActorSystem / Materializer を通した stream 実行だけを示し、ActorSystem なしの直実行 API には戻さない。

## Capabilities

### New Capabilities

- **`stream-island-actors`**
  - stream async boundary は island ごとの actor 実行境界になる。
  - island actor は独立した mailbox で drive command を処理する。
  - `async_with_dispatcher()` の dispatcher は該当 island actor の dispatcher 選択に反映される。
  - `Materialized::unique_kill_switch()` / `shared_kill_switch()` は複数 island から成る graph 全体を停止・中断できる。

### Modified Capabilities

- **`streams-backpressure-integrity`**
  - island 間 boundary の backpressure / completion / failure / cancellation を actor 分離後も保持する。
  - `WouldBlock` を同期直実行の失敗として扱わず、actor 駆動下の pending progress として検証する。

## Impact

**影響を受けるコード:**

- `modules/stream-core/src/core/impl/interpreter/island_splitter.rs`
- `modules/stream-core/src/core/materialization/actor_materializer.rs`
- `modules/stream-core/src/core/impl/materialization/stream_drive_actor.rs`
- `modules/stream-core/src/core/materialization/materialized.rs`
- `modules/stream-core/src/core/snapshot/materializer_state.rs`
- `modules/stream-core/src/core/impl/interpreter/boundary_sink_logic.rs`
- `modules/stream-core/src/core/impl/interpreter/boundary_source_logic.rs`
- `modules/stream-core/src/core/impl/interpreter/island_boundary.rs`
- `modules/stream-core/tests/*`
- `showcases/std/stream/*`

**公開 API 影響:**

- `Source::async_with_dispatcher()` / `Flow::async_with_dispatcher()` の dispatcher 指定が実行時に意味を持つようになる。
- `Materialized::unique_kill_switch()` / `shared_kill_switch()` は複数 island graph では graph 全体を代表する lifecycle surface になる。
- `MaterializerState::stream_snapshots()` は island ごとの診断経路として維持し、graph-wide lifecycle surface の代用にはしない。
- ActorSystem / Materializer なしの stream 実行入口は追加しない。既存の ActorSystem なし materializer helper は公開実行入口として残さず、削除または `cfg(test)` / `pub(crate)` のテスト専用 API へ縮小する。
- カスタム stream mailbox selector は本 change では追加しない。

**挙動影響:**

- 複数 island stream は 1 actor 直列 drive ではなく、island ごとの actor mailbox で進む。
- dispatcher を分けた island は actor runtime の dispatcher 分離を受ける。
- async boundary は throughput / fairness / failure propagation の観測単位になる。
- kill switch / materializer shutdown は graph 全体の island を横断して作用する。

## Non-goals

- stream operator DSL の追加や Pekko operator 全量実装。
- remote stream / cluster stream / distributed stream の実装。
- island ごとの OS thread 固定割り当て。
- stream 専用 mailbox selector API の追加。
- `Actor::receive` や mailbox drain contract の async 化。
- 旧直実行 API や ActorSystem なし execution helper の復活。
- lock-free / waker-driven boundary への最適化。
- dispatcher / actor 観測用の診断 API を公開面へ昇格すること。

> 前提: Pekko 互換仕様と Rust らしい設計の両立を、常に念頭に置いて判断する。
