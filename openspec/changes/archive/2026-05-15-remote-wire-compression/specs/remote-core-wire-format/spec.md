## ADDED Requirements

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

## MODIFIED Requirements

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
