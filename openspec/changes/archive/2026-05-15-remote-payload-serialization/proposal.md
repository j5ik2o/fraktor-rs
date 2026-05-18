## Why

現行の TCP remoting は `bytes::Bytes` と `Vec<u8>` payload だけを受け付けるため、actor-core に serializer が登録されていても、remote `ActorRef` へ通常の actor message を送れない。DeathWatch、remote deployment、compression はいずれも serialized message lane を前提にするため、最初にこの制約を外す。

## What Changes

- fraktor-native envelope wire format に serialized message metadata を追加する: serializer id、optional manifest、payload bytes。
- outbound TCP remoting で、現行の byte-only payload 抽出ではなく actor-core serialization を remote scope で使う。
- inbound TCP remoting で、local actor delivery 前に actor-core deserialization を行う。
- raw byte payload は actor-core builtin serializer が扱う `Vec<u8>` または `ByteString` を標準経路にする。`bytes::Bytes` は builtin serializer 対象ではないため、使う場合は caller が serializer を登録する。
- 未登録 payload や不正 payload は観測可能な失敗として扱う。empty bytes、debug text、型名文字列への黙った代替はしない。
- **BREAKING**: fraktor-native `EnvelopePdu` layout を変更する。pre-release の既存 remote wire 互換は保持しない。

## Capabilities

### New Capabilities

なし。

### Modified Capabilities

- `remote-core-wire-format`: Envelope frame は raw payload bytes だけでなく serialized-message metadata を運ぶ。
- `remote-core-extension`: inbound envelope frame は actor-core serialization で deserialize され、buffer される `InboundEnvelope` は復元済み `AnyMessage` を持つ。
- `remote-adaptor-std-tcp-transport`: TCP outbound envelope 変換は、登録済み `AnyMessage` payload に actor-core serialization registry を使う。

## Impact

- `modules/remote-core/src/wire/` の envelope PDU / codec。
- `modules/remote-core/src/extension/remote.rs` の inbound envelope 復元。
- `modules/remote-adaptor-std/src/transport/tcp/` の outbound conversion。
- `modules/remote-adaptor-std/src/extension_installer/` の local delivery bridge。
- `modules/actor-core-kernel/src/serialization/` の既存 `SerializationExtension`、`SerializationRegistry`、`SerializedMessage`、`SerializationCallScope::Remote` との接続。
- 既存の byte-only remote test は、`Vec<u8>` または `ByteString` に寄せて明示的な serializer path を通るように更新する。
