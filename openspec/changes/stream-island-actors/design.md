## Context

現在の stream async boundary は、graph construction と island splitting までは進んでいる。

- `Source::async()` / `Flow::async()` は最後の graph node に `AsyncBoundaryAttr` を付ける。
- `Source::async_with_dispatcher()` / `Flow::async_with_dispatcher()` は `AsyncBoundaryAttr` と `DispatcherAttribute` を付ける。
- `IslandSplitter` は async stage を upstream island の最後として扱い、次 stage 以降を downstream island に分ける。
- `SingleIslandPlan` は `dispatcher: Option<String>` を持つ。
- `ActorMaterializer` は複数 island に分かれた graph を island ごとの `Stream` / `StreamHandleImpl` に変換する。

ただし、現状の実行は Pekko の island architecture には届いていない。`ActorMaterializer::start()` は単一の `StreamDriveActor` を system actor として起動し、materialized island の全 handle をその actor に登録する。`StreamDriveActor::tick()` は自分の mailbox 内で全 handle を順番に `drive()` するため、island ごとの actor / mailbox / dispatcher は独立していない。

また、`SingleIslandPlan::dispatcher()` は存在するが、`SingleIslandPlan::into_stream_plan()` 後に dispatcher 情報は失われる。したがって `async_with_dispatcher()` の dispatcher 指定は、現在の runtime では island actor 起動に使われていない。

## Goals / Non-Goals

**Goals:**

- `async()` で分割された island を actor runtime 上の独立実行単位にする。
- 各 island が独立した actor mailbox 上で `drive()` されるようにする。
- `async_with_dispatcher()` の dispatcher を downstream island actor の `Props::with_dispatcher_id(...)` に反映する。
- island 間 boundary の backpressure / completion / failure / cancellation の契約を actor 分離後も保持する。
- materialized handle / materializer shutdown / snapshot が複数 island graph 全体を扱えるようにする。

**Non-Goals:**

- stream operator DSL の追加。
- `Actor::receive` や actor mailbox drain contract の async 化。
- island ごとの OS thread 固定割り当て。
- stream 専用 mailbox selector API の追加。
- remote stream / cluster stream の実装。
- ActorSystem なしの stream 直実行 API の復活。

## Decisions

### Decision 1: island ごとに 1 actor が 1 stream handle を所有する

`StreamDriveActor` が複数 handle を所有して直列 drive する構造をやめる。新しい実行単位は「1 island actor が 1 `StreamHandleImpl` を所有する」形にする。

想定する内部型名は以下とする。最終的なファイル配置は既存 package 整理に合わせる。

- `StreamIslandActor`
- `StreamIslandCommand`
- `StreamIslandHandle`
- `MaterializedStreamGroup`

これらは公開 API ではなく、`modules/stream-core` 内部の materialization 実装として扱う。

```text
現状:

ActorMaterializer
  -> StreamDriveActor
       -> drive(island 0 handle)
       -> drive(island 1 handle)
       -> drive(island 2 handle)

変更後:

ActorMaterializer
  -> StreamIslandActor(0) mailbox dispatcher A
  -> StreamIslandActor(1) mailbox dispatcher B
  -> StreamIslandActor(2) mailbox dispatcher C

island 0 --boundary--> island 1 --boundary--> island 2
```

### Decision 2: tick source は分離してよいが drive は island actor 内で行う

tick の供給方法は実装時に次のどちらかを選べる。

- 各 island actor に scheduler job を持たせ、scheduler が直接 `StreamIslandCommand::Drive` を送る。
- materializer ごとに tick fanout actor を持ち、fanout actor が各 island actor へ `Drive` command を送る。

どちらを選んでも、fanout actor や scheduler callback が `StreamHandleImpl::drive()` を直接呼んではならない。`drive()` は必ず対象 island actor の mailbox 内で実行する。

この制約により、dispatcher 分離と mailbox 分離が実行時意味論として守られる。

### Decision 3: dispatcher は async boundary の downstream island に適用する

`IslandSplitter` は現在、async stage の `DispatcherAttribute` を downstream stage の dispatcher candidate として扱っている。この意味論を維持し、`async_with_dispatcher("x")` は「その async boundary の下流 island を dispatcher `x` で実行する」と定義する。

materialization は、`SingleIslandPlan::dispatcher()` を `into_stream_plan()` の前に読み取り、island actor 用 `Props` に反映する。

```text
source --map.async_with_dispatcher("blocking")--> sink

island 0:
  source + map
  dispatcher: default

island 1:
  boundary source + sink
  dispatcher: blocking
```

dispatcher id が actor system に登録されていない場合、materialization は default dispatcher にフォールバックしてはならない。Pekko と同様に設定ミスとして失敗させ、失敗は `StreamError` として観測可能にする。

### Decision 4: boundary は actor 間の唯一の data channel とする

island 間の data flow は boundary を通してのみ行う。初期実装では既存の `IslandBoundaryShared` を使い続けてよい。ただし、actor 分離後に必要な状態遷移を満たせない場合は、同じ contract を持つ境界型へ置き換える。

boundary contract は以下とする。

- upstream island は boundary full を `WouldBlock` 相当の局所 pending として扱い、要素を失わない。
- downstream island は boundary empty かつ open を pending として扱い、busy loop しない。
- upstream completion は pending 要素の flush 後に downstream completion として観測される。
- upstream failure は downstream failure として観測される。
- downstream cancel は upstream island の cancel または shutdown command へ伝播する。

boundary は data channel であり、actor lifecycle command の唯一の配送経路ではない。downstream cancel を upstream actor へ伝える制御経路は `MaterializedStreamGroup` が持つ island actor refs、または同等の control plane で実装する。`IslandBoundaryShared` を継続利用する場合でも、少なくとも cancellation state を boundary に記録し、group が upstream island actor へ `Cancel` または `Shutdown` を送れるようにする。boundary full / empty の data state だけで downstream cancellation を表現してはならない。

### Decision 5: materialized graph は composite lifecycle を持つ

複数 island graph で利用者に返す handle は、先頭 island の handle だけではなく graph 全体を代表する composite handle として扱う。

必要な振る舞いは以下とする。

- `cancel()` は全 island actor へ cancel を伝える。
- terminal state は全 island の terminal state と boundary terminal state から導出する。
- snapshot は materialized graph 単位と island 単位の両方を観測できる。
- materialization 途中で island actor 起動に失敗した場合、起動済み island actor と boundary resource を rollback する。

`Materialized::handle()` が返す公開 handle は、この composite lifecycle を指す。既存の `StreamHandleImpl` を維持する場合は、内部状態を単一 `StreamShared` 専用から single / composite のどちらも表せる構造へ変更する。既存構造で無理に表現して先頭 island handle を返す実装は禁止する。

### Decision 6: ActorSystem なし materializer helper は公開実行入口にしない

island actor を起動するには ActorSystem が必須である。既存の `ActorMaterializer::new_without_system` 相当の helper は、本 change の完了時点で公開 runtime API として残してはならない。

扱いは次のどちらかに限定する。

- 削除する。
- `#[cfg(test)]` かつ `pub(crate)` に縮小し、materialization 実行ではなく unit test の構築補助だけに使う。

どちらの場合も、ActorSystem なしで `start()` / `materialize()` が成功する経路や、ActorSystem なし直実行 API を復活させてはならない。

### Decision 7: materializer shutdown は停止失敗を握りつぶさない

`ActorMaterializer::shutdown()` は materializer が所有する island actor、tick resource、boundary resource を決定的に停止する。停止中の partial failure は、best-effort コメントで黙殺せず、少なくとも返り値または actor error として観測できるようにする。

Drop など回復不能な best-effort 経路で戻り値を捨てる場合だけ、失敗しても契約が壊れない理由をコメントに明記する。

## Risks / Trade-Offs

**actor 数が増える:**

async boundary ごとに actor が増えるため、非常に細かい boundary を大量に置く stream は mailbox / actor overhead が増える。これは Pekko 互換の意味論として受け入れる。単一 fused island は引き続き最小構成で動く。

**boundary が lock ベースのままだと fairness が弱い可能性がある:**

初期実装で `IslandBoundaryShared` を維持する場合、actor は tick 駆動で再試行する。将来 waker / signal 型 boundary に置き換える余地は残すが、本 change では busy loop しないこと、要素を失わないこと、terminal signal を落とさないことを優先する。

**dispatcher test が実装依存になりやすい:**

単に値が流れることだけを検証すると退行を検出できない。test-only dispatcher factory または actor snapshot を使い、island actor が期待 dispatcher に attach されたことを観測する。

## Migration Plan

1. `StreamDriveActor` の責務を「複数 handle を直列 drive する actor」から取り除く。
2. `StreamIslandActor` と `StreamIslandCommand` を追加し、1 actor が 1 island handle を drive する形にする。
3. `ActorMaterializer` の materialization で、island ごとに actor を spawn する。
4. `SingleIslandPlan::dispatcher()` を actor `Props::with_dispatcher_id(...)` へ反映する。
5. composite handle / materialized graph state を導入し、cancel / snapshot / shutdown を graph 全体へ適用する。
6. downstream cancel を upstream actor へ届ける control plane を追加する。
7. `ActorMaterializer::new_without_system` 相当の公開 helper を削除またはテスト専用へ縮小する。
8. boundary backpressure / completion / failure / cancellation の regression tests を追加する。
9. showcase と stream tests を ActorSystem + Materializer 経由の実行契約へ寄せる。

## Open Questions

**tick fanout を actor にするか scheduler job を island ごとに持つか:**

どちらでも contract は満たせる。初期実装ではシンプルさを優先し、既存 scheduler API で安全に cancel できる方を選ぶ。

**snapshot の公開粒度:**

materializer snapshot に island actor id / dispatcher id / terminal state を含めるか、内部 test helper に限定するかは実装時に決める。少なくとも regression test が dispatcher 反映を観測できる経路は必要である。

**stream mailbox selector API:**

現在 stream attribute として mailbox selector は存在しない。本 change では actor mailbox が island ごとに独立することだけを保証し、カスタム mailbox selection は別 change とする。
