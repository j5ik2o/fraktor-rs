## Why

`add-remote-stream-ref-transport` の途中実装により、当初の change は考慮不足であることが分かった。問題は TLS や remote transport 単体ではなく、Pekko `StreamRefs.sourceRef` / `StreamRefs.sinkRef` 相当の materialization 意味論が未確定なまま、resolver、endpoint actor、protocol payload serializer を先に仕様化していた点にある。

Pekko では、stream graph を materialize した結果として `SourceRef` / `SinkRef` が得られ、その ref が remote message payload として渡り、受信側は同じ向きの ref として materialize する。fraktor-rs でもこの public workflow を先に固定しない限り、actor path string や endpoint actor infrastructure が動いても、backpressure、completion ordering、failure visibility の正しさを証明できない。

したがって、この change は「remote endpoint infrastructure を追加する」ではなく、「Pekko-compatible StreamRef materialization contract を local resolver round-trip で証明し、その contract の上に remote transport を接続する」change として修正する。

## What Changes

- `StreamRefs.source_ref` 相当は、producer 側 stream を materialize して `SourceRef<T>` を返す contract として定義する。serialized `SourceRef<T>` は受信側でも `SourceRef<T>` として resolve され、consumer が `into_source` / `source` 相当で読む。
- `StreamRefs.sink_ref` 相当は、consumer 側 sink を materialize して `SinkRef<T>` を返す contract として定義する。serialized `SinkRef<T>` は受信側でも `SinkRef<T>` として resolve され、producer が `into_sink` / `sink` 相当へ書く。
- `spawn_source_ref` / `spawn_sink_ref` や explicit actor path string は、必要なら std adaptor 内部または serializer support API に限定する。application-level public workflow の中心にしない。
- remote payload では、domain message が `SourceRef<T>` / `SinkRef<T>` を持つように見える contract を維持する。内部表現として canonical actor path string を使う場合も、serializer / resolver の実装詳細として扱う。
- local resolver round-trip を remote transport 前の必須 proof にする。local proof が pass してから、two-ActorSystem SourceRef / SinkRef integration と failure integration へ進む。
- `stream-core-kernel` は no_std と remote 非依存を維持し、remote ActorSystem / serialization registry / transport との接続は std adaptor または integration layer に閉じる。
- TLS stream API と汎用 TCP stream DSL は引き続き対象外とする。

## Capabilities

### New Capabilities

- `stream-ref-materialization-contract`: `SourceRef` / `SinkRef` の生成、serialization format 化、resolve 後の materialization の向きを Pekko 互換に固定する契約。
- `remote-stream-ref-transport`: materialization contract の上で `SourceRef` / `SinkRef` を remote message payload として渡し、remote boundary 越しに back-pressured stream handoff を成立させる契約。

### Modified Capabilities

- `streams-backpressure-integrity`: local resolver round-trip と remote boundary の両方で demand / element / terminal signal が同じ ordering と failure visibility を維持する要件を追加する。
- `remote-adaptor-std-provider-dispatch`: StreamRef resolver / serializer support が remote actor ref provider surface を利用して endpoint actor path を解決する要件を追加する。

## Impact

- `modules/stream-core-kernel/src/stream_ref/`
- `modules/stream-core-kernel/src/impl/streamref/`
- `modules/stream-core-kernel/src/dsl/stream_refs.rs`
- `modules/stream-adaptor-std/src/`
- `modules/remote-adaptor-std/src/provider/`
- `modules/actor-core-kernel/src/serialization/`
- `tests/e2e/` or `modules/stream-adaptor-std/tests/`
- `docs/gap-analysis/stream-gap-analysis.md`

The current partial implementation is not assumed to be salvageable. The next implementation should start from the revised contract and keep infrastructure code subordinate to passing materialization proofs.
