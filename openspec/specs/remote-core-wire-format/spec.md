# remote-core-wire-format Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: 独自 binary wire format の採用

`fraktor_remote_core_rs::domain::wire` モジュールは独自 binary format による wire encoding/decoding を実装し、`prost`/`protobuf` 系の依存を持たない SHALL。

#### Scenario: prost 非依存

- **WHEN** `modules/remote-core/Cargo.toml` を検査する
- **THEN** `prost`・`protobuf`・`prost-types` 等の protobuf 関連クレートが依存に含まれていない

#### Scenario: bytes クレートのみへの依存

- **WHEN** `modules/remote-core/src/wire/` 配下のすべての import を検査する
- **THEN** `bytes::Bytes`・`bytes::BytesMut`・`bytes::Buf`・`bytes::BufMut` が wire format の主たる buffer 表現として使われている

### Requirement: Codec trait の存在

`fraktor_remote_core_rs::domain::wire::Codec` trait が定義され、PDU 種別の encode/decode を抽象化する SHALL。これにより将来 L2 (Pekko Artery TCP wire 互換) codec を実装差し替えで追加可能となる。

#### Scenario: Codec trait の存在

- **WHEN** `modules/remote-core/src/wire/codec.rs` を読む
- **THEN** `pub trait Codec` または `pub trait Codec<T>` が定義され、`encode` / `decode` メソッドを宣言している

#### Scenario: encode メソッド

- **WHEN** `Codec::encode` の定義を読む
- **THEN** `fn encode(&self, value: &T, buf: &mut BytesMut) -> Result<(), WireError>` または同等のシグネチャが宣言されている

#### Scenario: decode メソッド

- **WHEN** `Codec::decode` の定義を読む
- **THEN** `fn decode(&self, buf: &mut Bytes) -> Result<T, WireError>` または同等のシグネチャが宣言されている

### Requirement: PDU 種別の網羅

`fraktor_remote_core_rs::domain::wire` は以下の PDU (Protocol Data Unit) を encode/decode する SHALL: `EnvelopePdu` (メッセージエンベロープ)、`HandshakePdu` (handshake req/rsp)、`ControlPdu` (制御メッセージ)、`AckPdu` (ack/nack)。

#### Scenario: EnvelopePdu の存在

- **WHEN** `modules/remote-core/src/wire/envelope_pdu.rs` を読む
- **THEN** `pub struct EnvelopePdu` または `pub enum EnvelopePdu` が定義されている

#### Scenario: HandshakePdu の存在

- **WHEN** `modules/remote-core/src/wire/handshake_pdu.rs` を読む
- **THEN** `pub struct HandshakePdu` または `pub enum HandshakePdu` が定義され、`HandshakeReq` と `HandshakeRsp` を表現できる

#### Scenario: ControlPdu の存在

- **WHEN** `modules/remote-core/src/wire/control_pdu.rs` を読む
- **THEN** `pub enum ControlPdu` が定義され、heartbeat、quarantine 通知、shutdown 通知等の制御メッセージを表現する

#### Scenario: AckPdu の存在

- **WHEN** `modules/remote-core/src/wire/ack_pdu.rs` を読む
- **THEN** `pub struct AckPdu` または `pub enum AckPdu` が定義され、system message ack-based delivery をサポートする

### Requirement: WireError 型

`fraktor_remote_core_rs::domain::wire::WireError` enum が定義され、wire format の encode/decode 失敗カテゴリを網羅する SHALL。

#### Scenario: WireError の存在

- **WHEN** `modules/remote-core/src/wire/wire_error.rs` を読む
- **THEN** `pub enum WireError` が定義され、`InvalidFormat`、`UnknownVersion`、`UnknownKind`、`Truncated`、`InvalidUtf8` 等のバリアントを含む

#### Scenario: core::error::Error の実装

- **WHEN** `WireError` の derive または impl ブロックを検査する
- **THEN** `Debug`、`Display`、`core::error::Error` (no_std 互換) が実装されている

### Requirement: ゼロコピー指向の API

`Codec::decode` および PDU 関連 API は可能な限り `Bytes` (refcounted byte slice) を活用し、不要な `Vec<u8>` 確保とコピーを避ける SHALL。

#### Scenario: Bytes の利用

- **WHEN** `wire/` 配下の decode 系 API シグネチャを検査する
- **THEN** payload 部分は `Bytes` または `&[u8]` で受け渡され、`Vec<u8>::from(...)` による全コピーを行っていない

### Requirement: round-trip テスト

各 PDU 型は encode → decode → 比較の round-trip unit test を持ち、core クレート単体で実行可能である SHALL。

#### Scenario: EnvelopePdu の round-trip テスト

- **WHEN** `modules/remote-core/src/wire/envelope_pdu.rs` (または対応する `tests.rs`) を読む
- **THEN** EnvelopePdu の encode → decode が元の値と一致することを検証する `#[test]` が存在する

#### Scenario: テスト実行と no_std build の両立

- **WHEN** `cargo test -p fraktor-remote-core-rs` を実行する
- **THEN** wire format の round-trip テストが成功する

- **WHEN** `cargo build -p fraktor-remote-core-rs --no-default-features` を実行する
- **THEN** wire format 実装を含む production コードが no_std 条件でビルド成功する

### Requirement: フレーム形式 (length-prefixed + version + kind)

すべての wire PDU はフレーム先頭に固定長のヘッダを持つ SHALL。ヘッダ構成:
- `length: u32` (big-endian 4 バイト) — `length` 自身を除いた後続バイト数
- `version: u8` (1 バイト) — wire format バージョン (初期値 `1`)
- `kind: u8` (1 バイト) — PDU 種別 (envelope / handshake / control / ack の discriminator)

decoder は:
- length が残バッファ長を超える場合 `WireError::Truncated` または `WireError::InvalidFormat` を返す
- version が未知の場合 `WireError::UnknownVersion` を返す
- kind が未知の場合 `WireError::UnknownKind` を返す

#### Scenario: 正常フレームの length/version/kind 解釈

- **WHEN** 正常な envelope frame (length=N, version=1, kind=envelope) を decode する
- **THEN** フレームヘッダが解釈され、`length` バイト分の payload が取り出されて `EnvelopePdu` に decode される

#### Scenario: 未知バージョンの拒否

- **WHEN** version byte を `0xFF` に書き換えた envelope frame を decode する
- **THEN** `Err(WireError::UnknownVersion)` が返る

#### Scenario: 未知 kind の拒否

- **WHEN** kind byte を未定義値 (例: `0xEE`) に書き換えたフレームを decode する
- **THEN** `Err(WireError::UnknownKind)` が返る

#### Scenario: length が buffer を超える場合の拒否

- **WHEN** `length` フィールドが残バッファ長より大きい値を持つフレームを decode する
- **THEN** `Err(WireError::Truncated)` または `Err(WireError::InvalidFormat)` が返る

#### Scenario: 切り詰められたフレームの拒否

- **WHEN** length 分の payload が揃っていない (buffer が短い) 状態で decode する
- **THEN** `Err(WireError::Truncated)` が返る

### Requirement: プリミティブ型の binary 表現

wire format 内のプリミティブ型は以下の固定表現で encode される SHALL。すべて **big-endian (network byte order)**。

| 型 | 表現 |
|---|---|
| `u8` | 1 バイト |
| `u16` | 2 バイト big-endian |
| `u32` | 4 バイト big-endian |
| `u64` | 8 バイト big-endian |
| `String` | `u32 length (BE) + UTF-8 bytes` (最大長 `u32::MAX`) |
| `Vec<u8>` / payload | `u32 length (BE) + bytes` |
| `Option<T>` | `u8 tag (0=None, 1=Some) + (Some の場合) T の encode` |
| `bool` | 1 バイト (0=false, 1=true) |

#### Scenario: String encoding の round-trip

- **WHEN** `"hello"` を含むフィールドを encode → decode する
- **THEN** `[0x00, 0x00, 0x00, 0x05, 0x68, 0x65, 0x6c, 0x6c, 0x6f]` に encode され、decode で `"hello"` に戻る

#### Scenario: String の最大長検査

- **WHEN** `u32::MAX` を超える長さの文字列を encode しようとする
- **THEN** `Err(WireError::InvalidFormat)` または同等のエラーが返る

#### Scenario: 不正な UTF-8 の拒否

- **WHEN** `length` フィールドは正しいが続く bytes が不正な UTF-8 シーケンスである状態で String を decode する
- **THEN** `Err(WireError::InvalidUtf8)` が返る

### Requirement: EnvelopePdu の binary レイアウト

`EnvelopePdu` は以下のフィールドを順に encode する SHALL。payload は actor-core `SerializedMessage` 相当の metadata と bytes に分解して保持し、raw application bytes だけを持つ layout であってはならない（MUST NOT）。

recipient path、sender path、serializer manifest は compression table により literal または table reference として encode できる `CompressedText` 表現を使う。codec は compressed metadata を wire 表現として round-trip できなければならない（MUST）。actor delivery または deserialization へ進む前に、transport は table reference を literal に復元しなければならない（MUST）。

```
+----------------------+---------------------------+
| field                | encoding                  |
+----------------------+---------------------------+
| frame header         | length(u32 BE)            |
|                      | + version(u8)             |
|                      | + kind(u8=0x01)           |
+----------------------+---------------------------+
| recipient_path       | CompressedText            |
| sender_path          | Option<CompressedText>    |
| correlation_hi       | u64 BE                    |
| correlation_lo       | u32 BE                    |
| priority             | u8 (0=System, 1=User)     |
| redelivery_sequence  | priority-scoped metadata  |
| serializer_id        | u32 BE                    |
| manifest             | Option<CompressedText>    |
| payload              | u32 length + bytes        |
+----------------------+---------------------------+
```

#### Scenario: EnvelopePdu の kind byte は 0x01

- **WHEN** `EnvelopePdu` を encode したフレームの kind byte を検査する
- **THEN** `0x01` である

#### Scenario: priority の値

- **WHEN** `OutboundPriority::System` の envelope を encode する
- **THEN** priority byte は `0x00` である

- **WHEN** `OutboundPriority::User` の envelope を encode する
- **THEN** priority byte は `0x01` である

#### Scenario: sender_path が None の場合

- **WHEN** `sender_path = None` の envelope を encode する
- **THEN** sender_path のバイト列は `[0x00]` (Option tag のみ) で始まる

#### Scenario: serialized message metadata は round-trip する

- **WHEN** `serializer_id = 7`、`manifest = Some("example.Manifest")`、`payload = b"hello"` を持つ `EnvelopePdu` を literal metadata で encode して decode する
- **THEN** decode 後の `EnvelopePdu` は同じ serializer id、manifest、payload bytes を保持する
- **AND** manifest は payload bytes に結合されず、独立した `Option<CompressedText>` として復元される

#### Scenario: manifest が None の場合

- **WHEN** `manifest = None` の `EnvelopePdu` を encode する
- **THEN** manifest field は `Option<CompressedText>` の `None` tag として encode される
- **AND** decode 後の manifest は `None` である

#### Scenario: actor path reference metadata は round-trip する

- **WHEN** recipient path が actor-ref table reference id `3`、sender path が literal actor path の `EnvelopePdu` を encode して decode する
- **THEN** decode 後の recipient path metadata は actor-ref table reference id `3` である
- **AND** sender path metadata は同じ literal actor path である

#### Scenario: manifest reference metadata は round-trip する

- **WHEN** manifest が manifest table reference id `5` の `EnvelopePdu` を encode して decode する
- **THEN** decode 後の manifest metadata は table reference id `5` である
- **AND** serializer id と payload bytes は変更されない

### Requirement: HandshakePdu の binary レイアウト

`HandshakePdu` は以下のフィールドを順に encode する SHALL。

```
+---------------------+------------------+
| field               | encoding         |
+---------------------+------------------+
| frame header        | length(u32 BE)   |
|                     | + version(u8)    |
|                     | + kind(u8=0x02   |
|                     |   for Req,       |
|                     |   0x03 for Rsp)  |
+---------------------+------------------+
| origin_system       | String           |
| origin_host         | String           |
| origin_port         | u16 BE           |
| origin_uid          | u64 BE           |
+---------------------+------------------+
```

#### Scenario: HandshakeReq の kind byte は 0x02

- **WHEN** `HandshakePdu::Req(..)` を encode したフレームの kind byte を検査する
- **THEN** `0x02` である

#### Scenario: HandshakeRsp の kind byte は 0x03

- **WHEN** `HandshakePdu::Rsp(..)` を encode したフレームの kind byte を検査する
- **THEN** `0x03` である

### Requirement: ControlPdu の binary レイアウト

`ControlPdu` は以下のフィールドで encode する SHALL。compression table advertisement と acknowledgement は control frame (`kind = 0x04`) の subkind として表現し、actor delivery 用 envelope と混同してはならない（MUST NOT）。

```
+----------------------+-------------------------------+
| field                | encoding                      |
+----------------------+-------------------------------+
| frame header         | length(u32 BE)                |
|                      | + version(u8)                 |
|                      | + kind(u8=0x04)               |
+----------------------+-------------------------------+
| subkind              | u8                            |
|                      | 0x00 Heartbeat                |
|                      | 0x01 Quarantine               |
|                      | 0x02 Shutdown                 |
|                      | 0x03 HeartbeatResponse        |
|                      | 0x04 FlushRequest             |
|                      | 0x05 FlushAck                 |
|                      | 0x06 CompressionAdvertisement |
|                      | 0x07 CompressionAck           |
| authority            | String                        |
| reason               | Option<String>                |
| variant payload      | subkind-specific bytes        |
+----------------------+-------------------------------+
```

`CompressionAdvertisement` payload は `table_kind: u8`、`generation: u64 BE`、`entry_count: u32 BE`、repeated `entry_id: u32 BE + literal: String` で構成する SHALL。`table_kind = 0x00` は actor-ref table、`0x01` は manifest table を表す。`CompressionAck` payload は `table_kind: u8` と `generation: u64 BE` を含む SHALL。

#### Scenario: ControlPdu::Heartbeat の subkind

- **WHEN** `ControlPdu::Heartbeat { .. }` を encode する
- **THEN** subkind byte は `0x00` である

#### Scenario: ControlPdu::Quarantine の subkind

- **WHEN** `ControlPdu::Quarantine { reason, .. }` を encode する
- **THEN** subkind byte は `0x01` で、`reason` は `Option<String>` の `Some` として encode される

#### Scenario: ControlPdu::FlushRequest の subkind

- **WHEN** `ControlPdu::FlushRequest { .. }` を encode する
- **THEN** subkind byte は `0x04` である
- **AND** payload は flush id、scope、lane id、expected ack count を含む

#### Scenario: ControlPdu::CompressionAdvertisement の subkind

- **WHEN** `ControlPdu::CompressionAdvertisement { table_kind, generation, entries, .. }` を encode する
- **THEN** subkind byte は `0x06` である
- **AND** payload は table kind、generation、entry count、entry id と literal の列を含む

#### Scenario: ControlPdu::CompressionAck の subkind

- **WHEN** `ControlPdu::CompressionAck { table_kind, generation, .. }` を encode する
- **THEN** subkind byte は `0x07` である
- **AND** payload は table kind と generation を含む

#### Scenario: unknown compression table kind は拒否する

- **WHEN** compression control payload の table kind が `0xFF` である
- **THEN** decode は `Err(WireError::InvalidFormat)` を返す

### Requirement: AckPdu の binary レイアウト

`AckPdu` は system message delivery の ack/nack を表現し、以下のレイアウトで encode される SHALL。

```
+---------------------+------------------+
| field               | encoding         |
+---------------------+------------------+
| frame header        | length(u32 BE)   |
|                     | + version(u8)    |
|                     | + kind(u8=0x05)  |
+---------------------+------------------+
| sequence_number     | u64 BE           |
| cumulative_ack      | u64 BE           |
| nack_bitmap         | u64 BE           |
+---------------------+------------------+
```

#### Scenario: AckPdu の kind byte は 0x05

- **WHEN** `AckPdu` を encode したフレームの kind byte を検査する
- **THEN** `0x05` である

### Requirement: kind discriminator の一意性

各 PDU 種別の kind byte は互いに重複しない SHALL。

| kind byte | PDU 種別 |
|---|---|
| `0x01` | EnvelopePdu |
| `0x02` | HandshakePdu::Req |
| `0x03` | HandshakePdu::Rsp |
| `0x04` | ControlPdu |
| `0x05` | AckPdu |

#### Scenario: kind の衝突なし

- **WHEN** 上記 5 種類の PDU をそれぞれ encode して kind byte を取り出す
- **THEN** 5 つの kind byte はすべて異なる値 (`0x01`〜`0x05`)

#### Scenario: 未定義 kind の decode 失敗

- **WHEN** kind byte が上記テーブルに存在しない値 (例: `0x06`, `0xEE`) のフレームを decode する
- **THEN** `Err(WireError::UnknownKind)` が返る

### Requirement: system envelope redelivery sequence metadata

`EnvelopePdu` は system priority envelope に ACK/NACK redelivery 用 sequence metadata を持つ SHALL。user priority envelope は redelivery sequence を持ってはならない（MUST NOT）。`correlation_id` は request/response correlation 用に残し、redelivery sequence と兼用してはならない（MUST NOT）。

#### Scenario: system envelope carries redelivery sequence

- **WHEN** system priority `EnvelopePdu` を encode する
- **THEN** frame は redelivery sequence number を metadata として含む
- **AND** decode 後の `EnvelopePdu` から同じ sequence number を取得できる

#### Scenario: user envelope omits redelivery sequence

- **WHEN** user priority `EnvelopePdu` を encode する
- **THEN** frame は redelivery sequence number を持たない
- **AND** decode 後の `EnvelopePdu` は redelivery sequence absent として扱われる

#### Scenario: system envelope without sequence is rejected

- **WHEN** priority が system で redelivery sequence が存在しない envelope frame を decode する
- **THEN** decoder または inbound remote boundary は `WireError::InvalidFormat` もしくは同等の observable error を返す
- **AND** actor-core delivery へ進めない

#### Scenario: ack references envelope sequence

- **GIVEN** inbound system envelope の redelivery sequence が `100` である
- **WHEN** 受信側 association が ACK を生成する
- **THEN** `AckPdu` の cumulative ack / NACK bitmap は envelope の redelivery sequence を基準に生成される
- **AND** envelope の `correlation_id` は ACK 計算に使われない

### Requirement: CompressedText wire 表現

wire format は actor ref と serializer manifest の compression target を `CompressedText` として encode / decode する SHALL。`CompressedText` は `tag: u8` により literal と table reference を区別し、`0x00` は `Literal(String)`、`0x01` は `TableRef(u32 BE)` を表す。未知 tag は `WireError::InvalidFormat` として拒否しなければならない（MUST）。

actor path 用 `CompressedText` は actor-ref compression table を参照し、serializer manifest 用 `CompressedText` は manifest compression table を参照しなければならない（MUST）。serializer id と payload bytes は `CompressedText` にしてはならない（MUST NOT）。

#### Scenario: literal compressed text は round-trip する

- **WHEN** `CompressedText::Literal("example.Manifest")` を encode して decode する
- **THEN** decode 後の値は同じ literal である

#### Scenario: table reference compressed text は round-trip する

- **WHEN** `CompressedText::TableRef(7)` を encode して decode する
- **THEN** decode 後の値は table reference id `7` である

#### Scenario: unknown compressed text tag は拒否する

- **WHEN** `CompressedText` tag に `0xFF` を持つ bytes を decode する
- **THEN** `Err(WireError::InvalidFormat)` が返る

