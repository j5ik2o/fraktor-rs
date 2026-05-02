> 前提: Pekko 互換仕様と Rust らしい設計の両立を、常に念頭に置いて判断する。

## Context

現在の stream async boundary は、graph construction と island splitting までは進んでいる。

- `Source::async()` / `Flow::async()` は最後の graph node に `AsyncBoundaryAttr` を付ける。
- `Source::async_with_dispatcher()` / `Flow::async_with_dispatcher()` は `AsyncBoundaryAttr` と `DispatcherAttribute` を付ける。
- `IslandSplitter` は async stage を upstream island の最後として扱い、次 stage 以降を downstream island に分ける。
- `SingleIslandPlan` は `dispatcher: Option<String>` を持つ。
- `ActorMaterializer` は複数 island に分かれた graph を island ごとの `Stream` / `StreamShared` に変換する。
- `MaterializerState::stream_snapshots()` は登録済み `StreamShared` ごとの snapshot を返し、multi-island graph の island 数をすでに観測できる。
- `BoundarySinkLogic` / `BoundarySourceLogic` は boundary full / empty / completion / failure の単体契約をすでに持つ。

ただし、現状の実行は Pekko の island architecture には届いていない。`ActorMaterializer::start()` は単一の `StreamDriveActor` を system actor として起動し、materialized island の全 `StreamShared` をその actor に登録する。`StreamDriveActor::tick()` は自分の mailbox 内で全 stream を順番に `drive()` するため、island ごとの actor / mailbox / dispatcher は独立していない。

また、`SingleIslandPlan::dispatcher()` は存在するが、`SingleIslandPlan::into_stream_plan()` 後に dispatcher 情報は失われる。したがって `async_with_dispatcher()` の dispatcher 指定は、現在の runtime では island actor 起動に使われていない。

さらに、`Materialized` の公開 surface は `unique_kill_switch()` / `shared_kill_switch()` だが、multi-island graph でも内部的には先頭 island の `StreamShared` に依存している。`BoundarySourceLogic::on_cancel()` も現状は boundary 完了へ寄せているだけで、actor 分離後に必要となる upstream actor への明示的な stop request ではない。`ActorMaterializer::new_without_system` もまだ公開 helper であり、最終形としては絞り込みが必要である。

## Goals / Non-Goals

**Goals:**

- `async()` で分割された island を actor runtime 上の独立実行単位にする。
- 各 island が独立した actor mailbox 上で `drive()` されるようにする。
- `async_with_dispatcher()` の dispatcher を downstream island actor の `Props::with_dispatcher_id(...)` に反映する。
- island 間 boundary の backpressure / completion / failure / cancellation の契約を actor 分離後も保持する。
- `Materialized::unique_kill_switch()` / `shared_kill_switch()`、materializer shutdown、snapshot diagnostics が複数 island graph 全体を扱えるようにする。

**Non-Goals:**

- stream operator DSL の追加。
- `Actor::receive` や actor mailbox drain contract の async 化。
- island ごとの OS thread 固定割り当て。
- stream 専用 mailbox selector API の追加。
- remote stream / cluster stream の実装。
- ActorSystem なしの stream 直実行 API の復活。

## Pekko Compatibility Definition

この change でいう「Pekko 互換」は、Pekko の内部クラス構造を 1:1 で再現することではなく、async island runtime の意味論を一致させることを指す。参照実装としては、少なくとも次を一次資料とする。

- `references/pekko/stream/src/main/scala/org/apache/pekko/stream/impl/fusing/ActorGraphInterpreter.scala`
- `references/pekko/stream/src/main/scala/org/apache/pekko/stream/SubscriptionWithCancelException.scala`

fraktor-rs 側の対応付けは次のとおりとする。

- `StreamIslandActor`
  Pekko の `ActorGraphInterpreter` / `GraphInterpreterShell` が担う actor-owned execution に対応する
- island crossing + `IslandBoundaryShared`
  Pekko の actor boundary event と publisher/subscriber boundary に対応する
- `Cancel(cause)`
  Pekko の `SubscriptionWithCancelException.cancel(cause)` に対応する downstream cancellation control plane とする
- graph-scoped kill switch / materializer shutdown / test-only snapshot
  Pekko の graph-wide lifecycle と `ActorGraphInterpreter.Snapshot` 相当の観測経路に対応する

dispatcher lookup については、Pekko の設定体系を完全再現することではなく、fraktor-rs として fail-closed を選ぶ。未登録 dispatcher を暗黙 fallback しない点は、Rust らしい安全側の設計としてこの change の仕様に含める。

## Completion Gate

この change は「基盤追加の途中段階」を completed とみなしてはならない。completed の判定は、次の core contract が同時に成立していることを条件とする。

- `async()` で分割された各 island が独立 actor / 独立 mailbox で drive される
- `async_with_dispatcher()` が downstream island actor に実際に反映される
- graph-scoped kill switch / materializer shutdown / terminal state が複数 island graph 全体を代表する
- downstream cancellation が boundary state だけでなく upstream island actor への control plane として伝播する
- ActorSystem なし materializer helper が公開 runtime API に残っていない
- 上記を観測する regression test と最終 CI が揃っている

逆に、次のどれかが残っているなら、この change は未完了である。

- island 分割だけ存在し、実行 actor が 1 個のまま
- dispatcher candidate を保持しているだけで actor spawn に反映されない
- kill switch が先頭 island にしか効かない
- downstream cancel が `boundary.complete()` 相当の局所表現に留まる
- `ActorMaterializer::new_without_system` が公開 runtime API として見えている

core capability を複数 change に割って先送りしてはならない（MUST NOT）。追加の将来拡張は non-goal として切り出してよいが、それは上記 completion gate を弱める理由にならない。

## Decisions

### Decision 0: Pekko 互換の island runtime は原子的に deliver する

この change は、Pekko 互換の island runtime を 1 つの capability として deliver する。実装順序は段階的でよいが、archive / completion 判定は atomic とする。つまり、actor / mailbox 分離、dispatcher 反映、graph-wide lifecycle、cancellation control plane、公開 API の絞り込みのすべてが揃うまで completed にしない。

そのため、実装中に追加 task が見つかった場合は、この change の tasks へ追加して扱う。core capability を「次の change でやる」として外へ逃がす設計判断は禁止する。

### Decision 1: island ごとに 1 actor が 1 `StreamShared` を所有する

`StreamDriveActor` が複数 stream を所有して直列 drive する構造をやめる。新しい実行単位は「1 island actor が 1 `StreamShared` を所有する」形にする。

想定する内部型名は以下とする。最終的なファイル配置は既存 package 整理に合わせる。

- `StreamIslandActor`
- `StreamIslandCommand`
これらは公開 API ではなく、`modules/stream-core` 内部の materialization 実装として扱う。`MaterializedStreamGraph` のような集約構造は Decision 5 の条件を満たす場合に限って導入する optional な内部型とする。

単一 island graph も特別扱いの runtime path を残さず、1 つの `StreamIslandActor` で実行する。つまり「fused 実行を維持する」とは、単一 interpreter のまま island actor 1 個で動くことを意味し、旧 `StreamDriveActor` 直列経路を残すことではない。

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

### Decision 2: tick source は island actor ごとの scheduler job に固定する

各 island actor は自分専用の scheduler job を持ち、scheduler は対象 island actor に対してだけ `StreamIslandCommand::Drive` を送る。materializer 全体で drive を扇形配信する tick fanout actor は採用しない。

scheduler callback が `StreamShared::drive()` を直接呼んではならない。`drive()` は必ず対象 island actor の mailbox 内で実行する。

`Drive` は coalescing command とする。各 island actor は「未処理の `Drive` が 0 または 1 個だけ存在する」ことを保証し、前回の `Drive` がまだ mailbox 内または実行中である間は次の `Drive` を enqueue しない。これにより tick 間隔より処理が遅い場合でも mailbox が `Drive` で無制限に膨らまない。

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

dispatcher id が actor system に登録されていない場合、materialization は default dispatcher にフォールバックしてはならない。fraktor-rs では fail-closed で設定ミスとして失敗させ、失敗は `StreamError` として観測可能にする。

### Decision 4: boundary は actor 間の唯一の data channel とし、cancellation は別 control plane で扱う

island 間の data flow は boundary を通してのみ行う。初期実装では既存の `IslandBoundaryShared` を使い続けてよい。ただし、actor 分離後に必要な状態遷移を満たせない場合は、同じ contract を持つ境界型へ置き換える。

boundary contract は以下とする。

- upstream island は boundary full を `WouldBlock` 相当の局所 pending として扱い、要素を失わない。
- downstream island は boundary empty かつ open を pending として扱い、busy loop しない。
- upstream completion は pending 要素の flush 後に downstream completion として観測される。
- upstream failure は downstream failure として観測される。
- downstream cancel は upstream island の `Cancel { cause: Option<StreamError> }` command へ伝播する。

control plane の意味は次のように固定する。

- `Cancel { cause: Option<StreamError> }`
  downstream island が「これ以上要素を必要としない」ことを upstream island へ伝える局所停止 command。`cause: None` は通常の active cancellation、`cause: Some(error)` は failure reason 付き cancellation を表す。graph 全体の graceful stop を意味しない。
- `Shutdown`
  kill switch `shutdown()` または `ActorMaterializer::shutdown()` による graph-wide graceful stop。既に受理済みの要素は可能な範囲で順序を保って drain する。
- `Abort(error)`
  kill switch `abort(error)` または actor failure による graph-wide failure stop。error を優先し、graceful drain は要求しない。

boundary は data channel であり、actor lifecycle command の唯一の配送経路ではない。downstream cancel を upstream actor へ伝える制御経路は `MaterializedStreamGraph` が持つ island actor refs、または同等の control plane で実装する。現在の `BoundarySourceLogic::on_cancel()` は `boundary.complete()` で局所停止へ寄せているが、actor 分離後はこれだけでは不十分である。completed runtime では `Cancel { cause }` control plane を唯一の上流伝播経路とし、`BoundarySourceLogic::on_cancel()` は残すとしても local boundary cleanup に限定し、upstream lifecycle を直接決めてはならない。`IslandBoundaryShared` を継続利用する場合でも、boundary full / empty の data state だけで downstream cancellation を表現してはならない。

また `IslandBoundaryShared` は island actor 間の共有境界として `Send + Sync` であり、要素キューと terminal state の更新は単一 critical section で観測可能でなければならない。fairness より correctness を優先し、actor 間の interleaving で要素ロスや二重配送が起きないことを test で固定する。

### Decision 5: materialized graph の lifecycle は graph-scoped kill switch state を優先して表す

複数 island graph で利用者に返す公開 surface は、現在の `Materialized::unique_kill_switch()` / `shared_kill_switch()` を維持しつつ、その意味を「先頭 island ではなく materialized graph 全体」に引き上げる。Rust らしい最小設計を優先し、まずは graph ごとに 1 つの `KillSwitchStateHandle` を共有して全 island `Stream` へ注入する案を第一選択とする。

共有には既存の `KillSwitchStateHandle` alias（`ArcShared<SpinSyncMutex<KillSwitchState>>`）をそのまま使い、graph ごとに 1 つの handle を全 island で共有する。新しい共有抽象を追加するのではなく、既存 kill switch 実装の共有モデルを踏襲する。

必要な振る舞いは以下とする。

- `shutdown()` / `abort()` は全 island actor へ伝わる。
- terminal state は全 island の terminal state と boundary terminal state から導出する。
- snapshot は materialized graph 単位と island 単位の両方を観測できる。
- materialization 途中で island actor 起動に失敗した場合、起動済み island actor と boundary resource を rollback する。

terminal state の集約規則は次の優先度に固定する。

1. 明示的な `Abort(error)` が観測された場合は、それが graph の最終失敗理由を決める
2. `Abort` がなければ、いずれかの island / boundary failure が graph の failure terminal state を決める
3. `Abort` / failure がなければ、graph-wide `Shutdown` または局所 `Cancel { cause: None }` による停止は completion より優先される
4. `Completed` は、全 island が正常完了し、全 boundary が drain 済みで、上記 1-3 の条件が存在しない場合にのみ観測される

複数 failure が競合した場合は first-cause を graph の代表 error とし、後続 error は診断情報としてのみ保持する。

graph-scoped kill switch state だけで表現しきれない actor ref / scheduler handle の集約が必要な場合に限り、`MaterializedStreamGraph` のような小さな内部構造を足す。Pekko 互換のために新しい公開 handle API を増やすことは最後の手段とし、先頭 island の `StreamShared` に公開意味論を背負わせる実装は禁止する。

dispatcher 検証と island actor 観測は、public snapshot API の拡張ではなく test-only actor snapshot / diagnostic 経路で行う。これは core completion gate の一部であり、実装依存の ad-hoc 観測で済ませてはならない。

### Decision 6: ActorSystem なし materializer helper は公開実行入口にしない

island actor を起動するには ActorSystem が必須である。既存の `ActorMaterializer::new_without_system` 相当の helper は、本 change の完了時点で公開 runtime API として残してはならない。

扱いは次のどちらかに限定する。

- 削除する。
- `#[cfg(test)]` かつ `pub(crate)` に縮小し、materialization 実行ではなく unit test の構築補助だけに使う。

どちらの場合も、ActorSystem なしで `start()` / `materialize()` が成功する経路や、ActorSystem なし直実行 API を復活させてはならない。

### Decision 7: materializer shutdown は停止失敗を握りつぶさない

`ActorMaterializer::shutdown()` は materializer が所有する island actor、tick resource、boundary resource を決定的に停止する。現在の実装は single drive actor への `Shutdown` 送信と scheduler cancel を行うが、tick cancel の結果や island 個別停止の失敗集約までは扱っていない。island actor 化後は停止中の partial failure を best-effort コメントで黙殺せず、少なくとも返り値または actor error として観測できるようにする。

materialization 中の rollback / shutdown 中の cleanup では、primary failure を返しつつ、後続 cleanup failure は必ず log または diagnostic に集約して観測可能にする。後続 failure を `let _ = ...` や無言コメントで捨ててはならない。Drop など回復不能な best-effort 経路で戻り値を捨てる場合だけ、失敗しても契約が壊れない理由をコメントに明記する。

## Risks / Trade-Offs

**actor 数が増える:**

async boundary ごとに actor が増えるため、非常に細かい boundary を大量に置く stream は mailbox / actor overhead が増える。これは Pekko 互換の意味論として受け入れる。単一 fused island は引き続き最小構成で動く。

**boundary が lock ベースのままだと fairness が弱い可能性がある:**

初期実装で `IslandBoundaryShared` を維持する場合、actor は tick 駆動で再試行する。将来 waker / signal 型 boundary に置き換える余地は残すが、本 change では busy loop しないこと、要素を失わないこと、terminal signal を落とさないことを優先する。

**dispatcher test が実装依存になりやすい:**

単に値が流れることだけを検証すると退行を検出できない。test-only actor snapshot / diagnostic を使い、island actor が期待 dispatcher に attach されたことを観測する。

**scheduler job 数が island 数に比例する:**

Decision 2 により scheduler job 数は materialized graph 数 × island 数になる。Pekko の self-scheduled `Resume` と完全一致ではないが、fraktor-rs では既存 scheduler API を活用して island actor の mailbox ownership を明確に保つことを優先する。負荷面の副作用は `Drive` coalescing と `drive_interval` 設定で抑制する。

## Migration Plan

1. `StreamDriveActor` の責務を「複数 stream を直列 drive する actor」から取り除く。
2. `StreamIslandActor` と `StreamIslandCommand` を追加し、1 actor が 1 island `StreamShared` を drive する形にする。
3. `ActorMaterializer` の materialization で、island ごとに actor を spawn する。
4. `SingleIslandPlan::dispatcher()` を actor `Props::with_dispatcher_id(...)` へ反映する。
5. graph-scoped `KillSwitchStateHandle` 注入、必要なら最小限の `MaterializedStreamGraph` を導入し、shutdown / abort / terminal aggregation / snapshot / rollback を graph 全体へ適用する。
6. downstream cancel を upstream actor へ届ける control plane を追加する。
7. `ActorMaterializer::new_without_system` 相当の公開 helper を削除またはテスト専用へ縮小する。
8. boundary backpressure / completion / failure / cancellation の regression tests を追加する。
9. showcase と stream tests を ActorSystem + Materializer 経由の実行契約へ寄せる。

> 前提: Pekko 互換仕様と Rust らしい設計の両立を、常に念頭に置いて判断する。
