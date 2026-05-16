## Context

Pekko `SourceRef` / `SinkRef` は remote boundary を越えて渡すための actor-backed reference であり、`StreamRefResolver` はそれらを actor path の serialization format に変換して復元する。fraktor-rs の `StreamRefs` は `StreamRefHandoff` による local handoff、protocol enum、settings、exception variants を持つが、remote resolver / serializer / actor transport 連携は存在しない。

remote 側は `StdRemoteActorRefProvider` による remote ActorRef materialization、`RemoteActorRefSender` による `RemoteEvent::OutboundEnqueued` 接続、serialization registry から envelope payload への変換、TCP transport、DeathWatch / `AddressTerminated` が揃っている。次の設計課題は、stream-core に remote 依存を入れずに StreamRef protocol を actor transport へ接続することである。

## Goals / Non-Goals

**Goals:**

- `SourceRef` / `SinkRef` を remote endpoint actor path として serialization format 化し、別 ActorSystem で remote-capable ref として復元できる契約を定義する。
- local handoff と remote handoff を分離し、`stream-core-kernel` の no_std 境界を維持する。
- StreamRef demand / element / completion / failure / cancellation を remote user payload として配送し、backpressure と terminal signal を失わない。
- remote partner termination、address termination、invalid partner、sequence mismatch を stream failure として観測可能にする。
- 既存 remote actor ref provider / transport / serialization 経路を利用し、StreamRef 専用の transport port を追加しない。

**Non-Goals:**

- TLS stream API、TLS option、TLS protocol message の実装。
- 汎用 `Tcp`, `IncomingConnection`, `OutgoingConnection` stream DSL の実装。
- Reactive Streams TCK / Java DSL / Scala implicit API の再現。
- remote transport wire format の互換拡張や StreamRef 専用 frame の追加。
- cluster sharding、pub-sub、persistence replay と StreamRef の統合。

## Decisions

### 決定 1: StreamRef remote endpoint は actor path で表現する

`SourceRef` / `SinkRef` の remote serialization format は、Pekko と同様に endpoint actor の canonical actor path string とする。binary token や独自 id table ではなく actor path を使うことで、既存 `ActorRefResolver` / `StdRemoteActorRefProvider` / remote watch hook の契約に乗せられる。

代替案として StreamRef 専用 registry id を remote wire に載せる方法があるが、remote actor ref 解決と DeathWatch を二重実装することになるため採用しない。

### 決定 2: `stream-core-kernel` は protocol と local semantics だけを持つ

`stream-core-kernel` は no_std のまま、StreamRef protocol、settings、sequence validation、local handoff、stream failure mapping を保持する。ActorSystem、remote ActorRef、tokio task、serialization registry、TCP transport は持たない。

remote endpoint actor の起動、actor path の serialization format 化、serialized path の resolve、remote protocol message の送受信は std 側または stream / remote integration layer の責務にする。core から `remote-core` へ直接依存する案は crate 境界を逆転させるため採用しない。

### 決定 3: StreamRef protocol は通常の remote user payload として配送する

`SequencedOnNext`、`CumulativeDemand`、`OnSubscribeHandshake`、completion、failure、ack は StreamRef protocol payload として serializer に登録し、通常の remote actor envelope 経由で配送する。`RemoteTransport` に StreamRef 専用 method や wire frame を追加しない。

これにより transport は既存の backpressure / retry / serialization failure / connection closed の観測経路を使える。専用 frame は高速化余地があるが、今は意味論を増やして検証面を広げるため採用しない。

### 決定 4: remote endpoint actor は partner を固定し、one-shot を守る

remote StreamRef は一度だけ partner と接続できる。最初の handshake で partner actor ref を固定し、以後は partner 以外からの protocol message を invalid partner failure として扱う。二重 materialization は成功させない。

これは Pekko の StreamRef が 1:1 pair であることに対応する。broadcast や multicast は複数の新しい StreamRef を作ることで表現し、単一 StreamRef の共有を許可しない。

### 決定 5: remote termination は stream failure へ写像する

partner actor の DeathWatch notification、remote address termination、transport-level connection closed は StreamRef endpoint に観測可能な failure として伝播する。partner actor termination は `RemoteStreamRefActorTerminated` 相当へ、address termination / connection closed は remote address または transport context を含む stream failure へ写像し、subscription timeout、invalid sequence、invalid partner と混同しない。

remote node failure を silent completion に写像する案は、要素喪失を正常終了として隠すため採用しない。

### 決定 6: TCP stream API は別 change に分離する

remote-adaptor-std には TCP transport 実装があるが、これは actor remoting envelope 用であり、Pekko Stream の汎用 byte stream TCP API とは責務が異なる。この change は StreamRef remote handoff に限定し、`Tcp` stream DSL と TCP error marker 型の実装は別 change で扱う。

## Risks / Trade-offs

- StreamRef endpoint actor と stream materializer lifecycle が循環しやすい -> endpoint actor は stream graph の materialized resource として所有し、shutdown / cancellation の責務を tasks で明確化する。
- protocol payload serializer が未登録だと remote delivery が失敗する -> std installer または integration setup が serializer registration を必須化し、未登録は materialization failure として観測する。
- remote backpressure と transport backpressure の区別が曖昧になる -> StreamRef demand は stream-level credit、transport backpressure は envelope enqueue failure として別エラー経路にする。
- DeathWatch / `AddressTerminated` と StreamRef completion が競合する -> failure は remote termination を優先し、正常 completion は protocol completion を sequence 通り受けた場合だけに限定する。
- actor path string format が将来変わる可能性がある -> resolver は actor-core の canonical path API に委譲し、StreamRef 独自 parser を持たない。

## Migration Plan

1. StreamRef remote endpoint / resolver の spec と tests を追加し、local handoff の既存 tests をベースラインとして維持する。
2. protocol payload serializer と remote endpoint actor の最小実装を std / integration layer に追加する。
3. `SourceRef` / `SinkRef` の local conversion と remote resolver conversion を分離し、二重 materialization / invalid partner / sequence failure tests を追加する。
4. two ActorSystem integration test で `SourceRef` と `SinkRef` を remote message payload として渡し、要素、demand、completion、failure が伝播することを検証する。
5. 実装後に `docs/gap-analysis/stream-gap-analysis.md` を更新し、StreamRef remote gap の状態を反映する。

rollback は active change を削除することで行う。pre-release のため legacy compatibility alias は追加しない。

## Open Questions

- endpoint actor を `stream-adaptor-std` に置くか、`stream-remote-adaptor-std` 相当の新 integration crate を作るかは実装時に Cargo 依存方向を確認して決める。
- StreamRef protocol serializer id の割り当て範囲は既存 built-in serializer id と衝突しない値を実装時に確定する。
