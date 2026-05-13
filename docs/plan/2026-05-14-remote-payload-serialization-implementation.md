# remote-payload-serialization implementation plan

## Goal

`openspec/changes/remote-payload-serialization` の 22 タスクを実装し、remote TCP 経路で任意の登録済み `AnyMessage` payload を actor-core serialization 経由で送受信できるようにする。

## Steps

1. OpenSpec の tasks と関連コードを再確認する。
   - verify: `openspec instructions apply --change remote-payload-serialization --json`
2. wire format と codec を更新する。
   - verify: `EnvelopePdu` が serializer id / manifest / payload bytes を round-trip する unit test
3. serialization extension の wiring を追加する。
   - verify: remote installer / transport / core が同じ `SerializationExtensionShared` を使う test
4. outbound serialization を実装する。
   - verify: `Vec<u8>` / `ByteString` / registered typed payload が serializer path を通り、未登録 payload と `bytes::Bytes` は失敗する test
5. inbound deserialization を実装する。
   - verify: `InboundEnvelope` は復元済み `AnyMessage` を持ち、decode 失敗時は buffer しない test
6. integration と gap docs を更新する。
   - verify: two-node TCP integration で `String` payload が round trip する
7. 対象検証を実行する。
   - verify: `cargo test -p fraktor-remote-core-rs`, `cargo test -p fraktor-remote-adaptor-std-rs`, `cargo build -p fraktor-remote-core-rs --no-default-features`
