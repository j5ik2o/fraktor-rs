## Why

stream の固定スコープで残る非 TLS ギャップは、`SourceRef` / `SinkRef` が local handoff に閉じており、Pekko `StreamRefResolver` 相当の remote serialization / actor transport 境界を持たない点に集中している。remote actor ref 解決、payload serialization、TCP transport、DeathWatch / `AddressTerminated` integration が揃ってきたため、StreamRef を remote boundary 越しに扱う change として切り出せる。

## What Changes

- `SourceRef` / `SinkRef` を serialization format へ変換し、remote endpoint ActorRef を介した ref として復元できる `StreamRefResolver` 相当の契約を追加する。
- local handoff と remote actor handoff を分離し、`stream-core-kernel` が `remote-core` / `remote-adaptor-std` に依存しない境界を定義する。
- StreamRef protocol message を remote user payload として配送できる serializer / manifest / dispatch contract を追加する。
- remote 側 StreamRef partner actor の termination、subscription timeout、invalid partner、sequence mismatch を stream failure として観測できるようにする。
- TLS stream API は対象外とし、汎用 `Tcp` stream DSL は別 change に分離する。

## Capabilities

### New Capabilities

- `remote-stream-ref-transport`: `SourceRef` / `SinkRef` を endpoint actor path と serialization format 経由で共有し、remote boundary 越しに back-pressured stream handoff を成立させる契約。

### Modified Capabilities

- `streams-backpressure-integrity`: remote boundary 越しの StreamRef demand / element / terminal signal が local stream と同じ backpressure と failure visibility を維持する要件を追加する。
- `remote-adaptor-std-provider-dispatch`: StreamRef resolver が remote actor ref provider surface を利用して remote StreamRef endpoint を materialize する要件を追加する。

## Impact

- `modules/stream-core-kernel/src/stream_ref/`
- `modules/stream-core-kernel/src/impl/streamref/`
- `modules/stream-core-kernel/src/dsl/stream_refs.rs`
- `modules/stream-adaptor-std/src/`
- `modules/remote-adaptor-std/src/provider/`
- `modules/actor-core-kernel/src/serialization/`
- `docs/gap-analysis/stream-gap-analysis.md`

`stream-core-kernel` は no_std を維持し、remote ActorSystem / TCP / task / serialization registry との接続は std adaptor または integration layer に閉じる。汎用 TCP stream API と TLS API はこの change では実装しない。
