## ADDED Requirements

### Requirement: 独自 binary wire format の採用

`fraktor_remote_core_rs::wire` モジュールは独自 binary format による wire encoding/decoding を実装し、`prost`/`protobuf` 系の依存を持たない SHALL。

#### Scenario: prost 非依存

- **WHEN** `modules/remote-core/Cargo.toml` を検査する
- **THEN** `prost`・`protobuf`・`prost-types` 等の protobuf 関連クレートが依存に含まれていない

#### Scenario: bytes クレートのみへの依存

- **WHEN** `modules/remote-core/src/wire/` 配下のすべての import を検査する
- **THEN** `bytes::Bytes`・`bytes::BytesMut`・`bytes::Buf`・`bytes::BufMut` が wire format の主たる buffer 表現として使われている

### Requirement: Codec trait の存在

`fraktor_remote_core_rs::wire::Codec` trait が定義され、PDU 種別の encode/decode を抽象化する SHALL。これにより将来 L2 (Pekko Artery TCP wire 互換) codec を実装差し替えで追加可能となる。

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

`fraktor_remote_core_rs::wire` は以下の PDU (Protocol Data Unit) を encode/decode する SHALL: `EnvelopePdu` (メッセージエンベロープ)、`HandshakePdu` (handshake req/rsp)、`ControlPdu` (制御メッセージ)、`AckPdu` (ack/nack)。

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

`fraktor_remote_core_rs::wire::WireError` enum が定義され、wire format の encode/decode 失敗カテゴリを網羅する SHALL。

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

`EnvelopePdu` は以下のフィールドを順に encode する SHALL。

```
+---------------------+------------------+
| field               | encoding         |
+---------------------+------------------+
| frame header        | length(u32 BE)   |
|                     | + version(u8)    |
|                     | + kind(u8=0x01)  |
+---------------------+------------------+
| recipient_path      | String           |
| sender_path         | Option<String>   |
| correlation_id      | u64 BE           |
| priority            | u8 (0=System,    |
|                     |    1=User)       |
| payload             | u32 length + bytes|
+---------------------+------------------+
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

`ControlPdu` は以下のフィールドで encode する SHALL。

```
+---------------------+------------------+
| field               | encoding         |
+---------------------+------------------+
| frame header        | length(u32 BE)   |
|                     | + version(u8)    |
|                     | + kind(u8=0x04)  |
+---------------------+------------------+
| subkind             | u8               |
|                     |   (0=Heartbeat,  |
|                     |    1=Quarantine, |
|                     |    2=Shutdown)   |
| authority           | String           |
| reason              | Option<String>   |
+---------------------+------------------+
```

#### Scenario: ControlPdu::Heartbeat の subkind

- **WHEN** `ControlPdu::Heartbeat { .. }` を encode する
- **THEN** subkind byte は `0x00` である

#### Scenario: ControlPdu::Quarantine の subkind

- **WHEN** `ControlPdu::Quarantine { reason, .. }` を encode する
- **THEN** subkind byte は `0x01` で、`reason` は `Option<String>` の `Some` として encode される

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
