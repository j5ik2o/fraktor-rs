## Why

Pekko の stream `async()` / `async(dispatcher)` は、fused graph を island に分け、island ごとに独立した actor / mailbox / dispatcher で実行するための境界である。一方、現在の fraktor-rs は `AsyncBoundaryAttr` による island 分割と boundary buffer は持っているが、materialization 後は複数 island の `StreamHandleImpl` を 1 つの `StreamDriveActor` が順番に drive している。

この状態では `async_with_dispatcher()` が dispatcher 属性を付けても実行時 dispatcher の選択に反映されず、Pekko 互換サンプルやテストが「ActorSystem 上で動いているが island は分離実行されていない」状態になる。正式リリース前に、stream island を実行単位として actor runtime に接続し、async boundary の意味論を Pekko に寄せて固定する。

## What Changes

### 1. stream island を actor 実行単位にする

`ActorMaterializer` は、`IslandSplitter` が生成した island ごとに独立した stream 実行 actor を起動する。各 actor は 1 つの island の `Stream` / `StreamHandleImpl` を所有し、自分の mailbox 上で drive / cancel / shutdown command を処理する。

単一 island の stream は現在と同じ fused 実行を維持してよい。ただし `async()` により複数 island へ分かれた graph は、1 つの drive actor に全 handle を登録するのではなく、island ごとの actor 群として materialize する。

### 2. dispatcher 属性を island actor 起動へ反映する

`Source::async_with_dispatcher()` / `Flow::async_with_dispatcher()` が付与した dispatcher は、`IslandSplitter` の island plan に保持されるだけでなく、該当 island actor の `Props` / dispatcher selector へ反映されなければならない。

dispatcher 指定がない island は actor system の default dispatcher を使う。stream 専用 mailbox 属性は現状存在しないため、本 change では「island ごとに actor mailbox が独立する」ことを対象にし、カスタム stream mailbox selector の追加は別 change とする。

### 3. island 間 boundary の backpressure / terminal propagation を契約化する

既存の `IslandBoundaryShared`、`BoundarySinkLogic`、`BoundarySourceLogic` を actor 間境界として使うか、同等の境界型に置き換える。どちらの場合も、以下の contract を固定する。

- boundary full は upstream island の局所 backpressure として扱う。
- boundary empty は downstream island の pending 状態として扱い、busy loop しない。
- upstream failure / completion は downstream island へ伝播する。
- downstream cancel は upstream island へ停止要求として伝播する。

### 4. materialized handle と materializer lifecycle を composite にする

複数 island の materialization では、利用者へ返す handle は全 island actor の lifecycle を代表する composite handle として振る舞う。cancel / shutdown / snapshot は、先頭 island だけでなく materialized graph 全体に作用する。

公開面では `Materialized::handle()` の戻り型を単に「先頭 island の `StreamHandleImpl`」として扱ってはならない。可能であれば既存の `StreamHandleImpl` を composite lifecycle を表す内部実装へ拡張し、型名を維持したまま複数 island graph 全体を代表させる。既存型名で表現できない場合は、本 change 内で公開 handle 型を破壊的に置き換え、先頭 island handle を返す互換経路は残さない。

`ActorMaterializer::shutdown()` は、materializer が起動した全 island actor と tick / drive resource を決定的に停止する。停止失敗は握りつぶさず、観測可能な `StreamError` または actor error として扱う。

### 5. tests と showcases を Pekko 互換の意味論へ寄せる

`async()` / `async_with_dispatcher()` のテストは、値が通るだけでなく、island 数、dispatcher 反映、boundary backpressure、cancel / failure propagation を検証する。showcase では ActorSystem / Materializer を通した stream 実行だけを示し、ActorSystem なしの直実行 API には戻さない。

## Capabilities

### New Capabilities

- **`stream-island-actors`**
  - stream async boundary は island ごとの actor 実行境界になる。
  - island actor は独立した mailbox で drive command を処理する。
  - `async_with_dispatcher()` の dispatcher は該当 island actor の dispatcher 選択に反映される。
  - materialized handle は複数 island から成る graph 全体を cancel / observe できる。

### Modified Capabilities

- **`streams-backpressure-integrity`**
  - island 間 boundary の backpressure / completion / failure / cancellation を actor 分離後も保持する。
  - `WouldBlock` を同期直実行の失敗として扱わず、actor 駆動下の pending progress として検証する。

## Impact

**影響を受けるコード:**

- `modules/stream-core/src/core/impl/interpreter/island_splitter.rs`
- `modules/stream-core/src/core/materialization/actor_materializer.rs`
- `modules/stream-core/src/core/impl/materialization/stream_drive_actor.rs`
- `modules/stream-core/src/core/impl/materialization/stream_handle*.rs`
- `modules/stream-core/src/core/impl/boundary*` または同等の island boundary 実装
- `modules/stream-core/tests/*`
- `showcases/std/stream/*`

**公開 API 影響:**

- `Source::async_with_dispatcher()` / `Flow::async_with_dispatcher()` の dispatcher 指定が実行時に意味を持つようになる。
- `Materialized::handle()` は複数 island graph では graph 全体を代表する composite handle を返す。先頭 island だけを cancel / snapshot する handle は公開しない。
- ActorSystem / Materializer なしの stream 実行入口は追加しない。既存の ActorSystem なし materializer helper は公開実行入口として残さず、削除または `cfg(test)` / `pub(crate)` のテスト専用 API へ縮小する。
- カスタム stream mailbox selector は本 change では追加しない。

**挙動影響:**

- 複数 island stream は 1 actor 直列 drive ではなく、island ごとの actor mailbox で進む。
- dispatcher を分けた island は actor runtime の dispatcher 分離を受ける。
- async boundary は throughput / fairness / failure propagation の観測単位になる。

## Non-goals

- stream operator DSL の追加や Pekko operator 全量実装。
- remote stream / cluster stream / distributed stream の実装。
- island ごとの OS thread 固定割り当て。
- stream 専用 mailbox selector API の追加。
- `Actor::receive` や mailbox drain contract の async 化。
- 旧直実行 API や ActorSystem なし execution helper の復活。
