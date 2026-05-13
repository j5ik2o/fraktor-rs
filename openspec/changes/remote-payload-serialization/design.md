## Context

現行 remote delivery は provider / event loop / TCP transport / inbound delivery bridge まで接続済みだが、TCP transport の payload contract は `bytes::Bytes` と `Vec<u8>` に限定されている。`TcpRemoteTransport::send` は `AnyMessage` を byte payload として downcast できない場合に `TransportError::SendFailed` を返し、inbound 側の `Remote::handle_inbound_envelope_pdu` は `EnvelopePdu::payload()` をそのまま `AnyMessage::new(Bytes)` に包んでいる。

actor-core-kernel には既に `SerializationExtension`、`SerializationRegistry`、`SerializedMessage`、`SerializationCallScope::Remote` がある。remote はこれを wire lane に接続し、通常の actor message を remote `ActorRef` 経由で送れるようにする。

## Goals / Non-Goals

**Goals:**

- registered serializer を持つ `AnyMessage` payload を remote TCP 経由で送受信できるようにする。
- outbound では `SerializationCallScope::Remote` で serialize し、inbound では local delivery 前に deserialize する。
- `EnvelopePdu` に serializer id / manifest / payload bytes を明示フィールドとして持たせる。
- 未登録 payload、manifest 不一致、decode 失敗は観測可能な error path に流す。
- `remote-core` の no_std 境界を維持する。

**Non-Goals:**

- Pekko Artery byte wire 互換。
- compression table / manifest compression / actor-ref compression。
- ACK/NACK redelivery、DeathWatch、remote deployment。
- 任意 closure や serializer 未登録型の自動 fallback。
- actor-core-kernel に `bytes::Bytes` builtin serializer を追加すること。標準の byte payload は既存 builtin の `Vec<u8>` または `ByteString` を使う。
- `AnyMessage` の全 flag を remote wire contract に載せること。sender は既存 sender path を使い、priority は既存 `OutboundPriority` を使う。

## Decisions

### Decision 1: serialization extension を remote installer から transport / core に渡す

`RemotingExtensionInstaller` は install 時に actor system から `default_serialization_extension_id()` に対応する `SerializationExtensionShared` を取得し、`TcpRemoteTransport` と `Remote` の両方に渡す。caller supplied の `SerializationExtensionInstaller`（custom setup）が remote installer より先に登録済みならその instance を使い、未登録なら同じ extension id で default extension を登録して使う。`ActorSystem` 側の default serialization extension 登録は installers 実行後に走るため、remote installer が先に default を登録した場合は後続の default 登録は既存 instance を再利用する。custom serializer を使う caller は、serialization installer を remoting installer より先に `ExtensionInstallers` へ登録する必要がある。

理由:

- outbound serialization は `TcpRemoteTransport` が `OutboundEnvelope` を `EnvelopePdu` へ変換する箇所で必要になる。
- inbound deserialization は `Remote::handle_inbound_envelope_pdu` が `InboundEnvelope` を buffer する前に行うと、local delivery bridge に serialized payload の特別扱いを持ち込まずに済む。
- `SerializationExtensionShared` は actor-core-kernel の型なので、`remote-core` の no_std 境界に std 実行基盤を持ち込まない。

代替案:

- local delivery bridge で `SerializedMessage` を検出して deserialize する案は、`InboundEnvelope` が一時的に「未復元 payload」を持つことになり、bridge に payload codec 分岐が漏れるため採用しない。
- `RemoteTransport::send` の引数を bytes に変える案は、core port が envelope 単位で送る既存責務を壊すため採用しない。

### Decision 2: `EnvelopePdu` は `SerializedMessage::encode()` blob ではなく明示フィールドを持つ

wire layout は `serializer_id: u32`、`manifest: Option<String>`、`payload: bytes` を envelope fields として encode する。`SerializedMessage::encode()` の結果を opaque payload として入れない。

理由:

- fraktor-native wire の primitive encoding は big-endian と length-prefix に統一されている。
- manifest compression や serializer id table を後続 change で適用する時、metadata が wire field として分かれていた方が責務境界が明確になる。
- `SerializedMessage` の内部 encode 形式に remote wire compatibility を結合しない。

### Decision 3: raw bytes も serializer path を通す

`Vec<u8>` / `ByteString` の byte payload は登録済み serializer を通る payload として扱う。`bytes::Bytes` の特別 fast path は削除し、caller が serializer を明示登録していない場合は observable failure にする。

理由:

- 「byte payload だけは別 contract」という例外を残すと、後続の compression / manifest handling が二重化する。
- actor-core の builtin serializer は `Vec<u8>` と `ByteString` を扱えるため、標準の byte payload は actor-core 側の型へ寄せる。

### Decision 4: inbound deserialization failure は envelope を buffer しない

`Remote::handle_inbound_envelope_pdu` は deserialization に失敗した envelope を `InboundEnvelope` として buffer しない。failure は `RemotingError::CodecFailed` または serialization error を保持できる remote error に mapping し、log / test observable path に流す。

理由:

- local actor mailbox に壊れた payload を渡してから失敗させるより、remote wire boundary で拒否する方が原因が明確になる。
- delivery bridge は「local actor ref 解決と `try_tell`」に集中できる。

## Risks / Trade-offs

- [Risk] serialization extension の取得順序を誤ると custom serializer が反映されない → `RemotingExtensionInstaller` は既存 extension を優先し、未登録時だけ default を登録する test を追加する。
- [Risk] caller が custom serialization installer を remoting installer より後に登録すると default setup の extension が先に固定され、後続 installer の custom setup は既存 instance 再利用になる → tasks で installer order の test と documentation を追加する。
- [Risk] `Remote::handle_remote_event` の write lock 中に deserialization が走る → actor delivery は従来通り lock 外で行い、deserialization 範囲は inbound frame 1 件に限定する。
- [Risk] wire layout 変更で既存 remote test が壊れる → pre-release 方針に従い互換 shim は作らず、test fixture を新 layout に更新する。
- [Risk] serializer 未登録型の失敗が transport failure と区別しにくい → `TransportError::SendFailed` だけで握りつぶさず、log または test-observable error assertion を追加する。
