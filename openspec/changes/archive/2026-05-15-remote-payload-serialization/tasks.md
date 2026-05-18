## 1. Wire Format

- [x] 1.1 `EnvelopePdu` に `serializer_id` / `manifest` / serialized payload bytes を追加し、raw payload only layout を削除する
- [x] 1.2 `EnvelopeCodec` の encode / decode を新 layout に更新し、metadata round-trip と `manifest = None` の unit test を追加する
- [x] 1.3 既存の wire / TCP test fixture を新しい `EnvelopePdu` constructor と layout に合わせて更新する

## 2. Serialization Wiring

- [x] 2.1 `RemotingExtensionInstaller` が actor system の `SerializationExtensionShared` を取得し、未登録時だけ default serialization extension を登録する
- [x] 2.2 `TcpRemoteTransport` に serialization extension shared handle を渡す constructor / builder 経路を追加する
- [x] 2.3 `Remote` に inbound deserialization 用の serialization extension shared handle を渡す constructor 経路を追加し、`RemoteShared::new(remote)` は薄い wrapper のまま保つ
- [x] 2.4 custom setup を持つ `SerializationExtensionInstaller` が先に登録済みの場合、remote installer が同じ instance を使う test を追加する
- [x] 2.5 custom serialization installer は remoting installer より先に登録する必要があることを test または docs で明示する

## 3. Outbound Serialization

- [x] 3.1 `TcpRemoteTransport::send` の envelope-to-PDU 変換を `SerializationCallScope::Remote` による serialization へ置き換える
- [x] 3.2 `Vec<u8>` / `ByteString` の byte payload が serializer path を通る test に更新する
- [x] 3.3 serializer 未登録 payload が元 envelope を保持した observable failure になる test を追加する
- [x] 3.4 connected peer 送信 test で serializer id / manifest / payload bytes が frame に入ることを確認する
- [x] 3.5 `bytes::Bytes` は custom serializer 未登録なら raw fast path に fallback せず失敗する test を追加する

## 4. Inbound Deserialization

- [x] 4.1 `Remote::handle_inbound_envelope_pdu` で `EnvelopePdu` metadata から `SerializedMessage` 相当を構築する
- [x] 4.2 inbound payload を actor-core serialization で deserialize し、`AnyMessage::from_erased` または同等の経路で復元済み `AnyMessage` を持つ `InboundEnvelope` を buffer する
- [x] 4.3 deserialization failure 時に `InboundEnvelope` を buffer せず、error / log path で観測できる test を追加する
- [x] 4.4 local delivery bridge が `SerializedMessage` や raw bytes の特別扱いを持たないことを確認する

## 5. Integration Verification

- [x] 5.1 two-node TCP integration test で `String` など登録済み serializer payload が remote `ActorRef` 経由で round trip することを確認する
- [x] 5.2 `cargo test -p fraktor-remote-core-rs` を実行する
- [x] 5.3 `cargo test -p fraktor-remote-adaptor-std-rs` を実行する
- [x] 5.4 `cargo build -p fraktor-remote-core-rs --no-default-features` を実行し、remote-core の no_std 境界を確認する
- [x] 5.5 実装完了時に `docs/gap-analysis/remote-gap-analysis.md` の該当 gap を更新する
