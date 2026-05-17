## Context

Pekko `SourceRef` / `SinkRef` は remote boundary を越えて渡せる actor-backed reference である。重要なのは actor path string そのものではなく、stream graph を materialize した結果として ref が得られ、その ref を受け取った側が同じ向きの ref として materialize できる点にある。

途中実装では resolver、endpoint actor、protocol payload serializer、failure mapping の infrastructure は進んだ。しかし materialized `SourceRef` / `SinkRef` が actor-backed endpoint として何を保持し、誰に所有され、state change 後にどう stream を再駆動するかが未決だったため、local proof が要素を完了まで運べず stall した。これは remote transport の不具合ではなく、remote-capable endpoint 表現と materialization contract が未確定であることを示している。

この design は、remote transport 実装より先に materialization semantics を確定するための修正版である。

## Goals / Non-Goals

**Goals:**

- Pekko `StreamRefs.sourceRef` / `StreamRefs.sinkRef` 相当の public workflow を fraktor-rs の型と crate 境界に合わせて定義する。
- serialized `SourceRef<T>` は `SourceRef<T>` として、serialized `SinkRef<T>` は `SinkRef<T>` として resolve される向きを固定する。
- actor-backed endpoint ref の local proof で endpoint ownership、canonical path 化、provider dispatch 経由の resolve、element、backpressure、completion、failure、cancellation が end-to-end に流れることを remote transport 前の必須 gate にする。
- `SourceRef` / `SinkRef` を application-level payload として渡せるように見える contract を維持し、actor path string は serializer / resolver support の内部表現に留める。
- `stream-core-kernel` の no_std 境界を維持し、remote ActorSystem / task / serialization registry / TCP transport 接続は std adaptor または integration layer に閉じる。
- remote partner termination、address termination、transport connection loss、invalid partner、invalid demand、sequence mismatch を stream failure として観測可能にする。

**Non-Goals:**

- TLS stream API、TLS option、TLS protocol message の実装。
- 汎用 `Tcp`, `IncomingConnection`, `OutgoingConnection` stream DSL の実装。
- Reactive Streams TCK / Java DSL / Scala implicit API の再現。
- StreamRef 専用 transport port、transport method、wire frame の追加。
- cluster sharding、pub-sub、persistence replay と StreamRef の統合。
- application code に actor path string を直接扱わせる workflow の推奨。

## Decisions

### 決定 1: Public workflow は Pekko の materialization 向きに合わせる

`SourceRef` は producer 側 stream を `StreamRefs.source_ref` 相当で materialize して得る。受信側は serialized `SourceRef` を `SourceRef` として resolve し、`into_source` / `source` 相当で consume する。

`SinkRef` は consumer 側 sink を `StreamRefs.sink_ref` 相当で materialize して得る。受信側は serialized `SinkRef` を `SinkRef` として resolve し、`into_sink` / `sink` 相当へ produce する。

`spawn_source_ref -> resolve_sink_ref` や `spawn_sink_ref -> resolve_source_ref` のように向きが反転して見える API は、public contract にしない。内部実装で partner endpoint を作る必要がある場合も、外側から見える名前とテストは `SourceRef` resolves to `SourceRef`、`SinkRef` resolves to `SinkRef` に揃える。

### 決定 2: Actor-backed endpoint proof を remote 前の gate にする

resolver / serializer は、serialize できる実体として actor-backed endpoint ref が存在して初めて意味を持つ。そのため、remote ActorSystem をまたぐ前に、同一 ActorSystem 内で以下を通常テストとして pass させる。

- producer stream -> actor-backed materialized `SourceRef<T>` -> canonical endpoint actor path -> provider dispatch -> resolved `SourceRef<T>` -> consumer sink
- consumer sink -> actor-backed materialized `SinkRef<T>` -> canonical endpoint actor path -> provider dispatch -> resolved `SinkRef<T>` -> producer stream

この proof では、endpoint actor ownership、one-shot pairing、demand 到着前の element 保持、accepted element と completion の順序、failure / cancellation の伝播、stream interpreter の wake / drive が成立していることを確認する。local handoff を in-process map や placeholder path string に保存するだけの proof は、この gate を満たさない。ここが通らない場合、remote integration に進まない。

### 決定 3: Actor path string は internal serialization format とする

Pekko と同様に endpoint actor の canonical actor path string を serialization format に使う。ただし、これは serializer / resolver support の内部表現であり、application-level workflow の中心には置かない。

Rust では `SourceRef<T>` / `SinkRef<T>` が generic で、内部に endpoint abstraction を持ち、`stream-core-kernel` は remote / std 非依存を維持する必要がある。そのため、typed ref object を core で直接 remote serializer に載せるのではなく、std adaptor または domain message serializer が resolver support を使って actor path string へ変換する。

この方針でも、ユーザーから見える contract は「domain message が `SourceRef<T>` / `SinkRef<T>` を持ち、remote 側で同じ ref として使える」ことである。明示 format は lower-level test と serializer support API に限定する。

### 決定 4: `stream-core-kernel` は protocol semantics を持ち、remote wiring は持たない

`stream-core-kernel` は no_std のまま、StreamRef settings、protocol model、sequence validation、demand validation、terminal ordering、local handoff、stream failure mapping を保持する。ActorSystem、remote ActorRef、tokio task、serialization registry、TCP transport は持たない。

remote endpoint actor の起動、canonical path format 化、serialized path の resolve、StreamRef protocol payload serializer registration、remote watch integration は std adaptor または integration layer の責務にする。

### 決定 5: StreamRef protocol は通常の remote actor payload として配送する

`OnSubscribeHandshake`、`CumulativeDemand`、`SequencedOnNext`、completion、failure、ack / cancellation は StreamRef protocol payload として serializer に登録し、通常の remote actor envelope 経由で配送する。`RemoteTransport` に StreamRef 専用 method や wire frame は追加しない。

transport backpressure は envelope enqueue / connection failure として扱い、stream-level demand と混同しない。accepted element を silently drop する実装は不可とする。

実装では StreamRef protocol serializer id を `41` とし、Pekko `StreamRefSerializer` と同じ manifest 文字列 `A/B/C/D/G/H` を protocol message 型に割り当てる。`SequencedOnNext` の element は raw `DynValue` ではなく nested `SerializedMessage` として保持し、typed domain payload をどの serializer で包むかは 5.2 の責務として残す。

typed `SourceRef<T>` / `SinkRef<T>` は generic `TypeId` と ActorSystem-bound resolve が必要なため、全 `T` に対する global serializer binding にはしない。domain message serializer は `StreamRefResolver` の field helper を使い、`SourceRef` を manifest `E`、`SinkRef` を manifest `F` の nested `SerializedMessage` に変換する。復号側は同じ helper から provider dispatch 経由で typed ref を復元する。

remote 側は `RemoteActorRefSender` が通常の `RemoteEvent::OutboundEnqueued` を作り、`TcpRemoteTransport` の `outbound_envelope_to_pdu` が既存の `SerializationExtension` で payload を bytes 化する経路を使う。`remote-core` / `remote-adaptor-std` に StreamRef 専用 `RemoteTransport` method や wire frame は追加しない。

### 決定 6: Endpoint actor は materialized stream resource に従属する

remote endpoint actor は stream graph の materialized resource として所有される。one-shot partner pairing を守り、最初の valid handshake で partner を固定し、二重 materialization や partner 以外からの protocol message は observable failure にする。

endpoint actor state だけが更新されて stream interpreter が進まない状態を避けるため、handshake、demand、element、terminal、failure、cancellation の state change は materialized stream を再駆動する wake / drive contract を持つ。

### 決定 7: Remote termination は normal completion にしない

partner DeathWatch、remote address termination、transport connection loss は StreamRef failure へ写像する。normal completion は protocol completion を sequence 通りに受け取り、pending element がすべて観測された場合だけ成立する。

local handoff proof では、`SequencedOnNext` は累積 demand が記録されるまで `WouldBlock` として保持し、pending element が残っている間は normal completion を先に返さない。cancellation は `CancellationCause::NoMoreElementsNeeded` として観測させ、`Ok(None)` の normal completion には潰さない。endpoint cleanup は paired partner がある場合に `StreamRefRemoteStreamFailure("NoMoreElementsNeeded")` を通常 actor message として送り、送信失敗は cleanup failure として返す。

remote actor enqueue backpressure は通常の actor envelope 送信失敗として扱う。既存 `RemoteActorRefSender` は full / closed channel 時に元 payload を `SendError` として返すため、StreamRef protocol payload も remote transport へ silently drop されない。ただし cancellation signal が two-ActorSystem 境界を越えることの実証は、7.x の remote integration で扱う。

## Risks / Trade-offs

- Rust の generic `SourceRef<T>` / `SinkRef<T>` と type-erased actor serialization registry の相性が悪い -> built-in core serializer ではなく、std adaptor の resolver support と domain message serializer の責務を明確化する。
- explicit actor path string helper が public workflow として広がる -> docs / tests では typed ref payload workflow を主経路にし、format tests は lower-level contract として分離する。
- actor-backed local proof を先に要求すると remote 実装の着手が遅れる -> fake resolver proof と remote transport failure を混同しないための gate として必要。
- endpoint actor と stream interpreter の lifecycle が循環しやすい -> materialized resource ownership、deterministic shutdown、wake / drive signal を tasks で明示する。
- DeathWatch / `AddressTerminated` と protocol completion が競合する -> remote termination before accepted protocol completion は failure を優先する。

## Migration Plan

1. 現在の途中実装を前提にせず、Pekko `StreamRefs.sourceRef` / `StreamRefs.sinkRef` と fraktor-rs の既存 local StreamRef 実装を再確認する。
2. `SourceRef` / `SinkRef` の public materialization workflow、resolver direction、remote-capable endpoint 表現を tests で固定する。
3. endpoint actor ownership、one-shot pairing、deterministic shutdown、wake / drive contract を std adaptor / integration layer 側で成立させる。
4. actor-backed local resolver proof を通常テストとして追加し、canonical path 化、provider dispatch、element、backpressure、completion、failure、cancellation を pass させる。
5. local proof の後に、serializer support、protocol payload registration、remote watch integration を std adaptor / integration layer に接続する。
6. two-ActorSystem integration test で typed `SourceRef` / `SinkRef` payload workflow を検証する。
7. remote failure integration test を追加する。
8. 実装後に `docs/gap-analysis/stream-gap-analysis.md` と tasks を更新する。

Rollback は active change の実装差分を破棄することで行う。pre-release のため legacy compatibility alias は追加しない。

## Open Questions

- endpoint actor を `stream-adaptor-std` に置くか、stream / remote integration crate を新設するかは、actor-backed endpoint 表現を決める時点で Cargo 依存方向を確認して決める。
