## Why

`EventStream` の subchannel 実装は classifier による配送絞り込みを満たしているが、late subscriber の replay と live publish の観測順序、`publish()` return 時点の同期契約、subscriber callback panic 時の購読ライフサイクルが仕様として未定義のまま残っている。これらを未規定にしたままだと、subscriber 実装や remote / cluster 側の診断 subscriber が、偶然の実装挙動に依存しやすくなる。

## What Changes

- `EventStreamShared::subscribe_with_key` は、登録時点で確定した buffered replay を `subscribe_with_key` が return する前に同期通知する契約として明文化する。
- `subscribe_with_key` と同時に別 thread から実行される `publish()` については、replay と live event の厳密な cross-thread 順序を保証しない契約として明文化する。
- `EventStreamShared::publish` は、panic が発生しない限り、対象 subscriber への callback が完了してから return する同期配送契約として明文化する。
- subscriber callback panic は catch / isolate / automatic unsubscribe しない。panic は呼び出し元へ伝播し、subscription lifecycle は自動変更しない。
- 上記契約に対応する tests と rustdoc を追加する。

## Capabilities

### New Capabilities

なし。

### Modified Capabilities

- `pekko-eventstream-subchannel`: subchannel event stream の replay/live ordering、publish observation、subscriber panic lifecycle の契約を追加する。

## Impact

- `openspec/specs/pekko-eventstream-subchannel/spec.md` への delta spec
- `modules/actor-core-kernel/src/event/stream/base_test.rs`
- `modules/actor-core-kernel/src/event/stream/event_stream_shared.rs`
- `modules/actor-core-kernel/src/event/stream/event_stream_subscriber_shared.rs`

`actor-core-kernel` は no_std を維持する。panic isolation のための `std::panic::catch_unwind` は導入しない。
