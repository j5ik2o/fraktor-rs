## MODIFIED Requirements

### Requirement: ControlPdu の binary レイアウト

`ControlPdu` は以下の共通フィールドと variant 固有フィールドで encode する SHALL。flush request / ack は actor payload ではなく control PDU として表現し、`EnvelopePdu` や actor-core serialization を経由してはならない（MUST NOT）。

```
+---------------------+-------------------+
| field               | encoding          |
+---------------------+-------------------+
| frame header        | length(u32 BE)    |
|                     | + version(u8)     |
|                     | + kind(u8=0x04)   |
+---------------------+-------------------+
| subkind             | u8                |
|                     |   (0=Heartbeat,   |
|                     |    1=Quarantine,  |
|                     |    2=Shutdown,    |
|                     |    3=HeartbeatRsp,|
|                     |    4=FlushRequest,|
|                     |    5=FlushAck)    |
| authority           | String            |
| reason              | Option<String>    |
| variant payload     | subkind ごとの    |
|                     | 固定順 field      |
+---------------------+-------------------+
```

`FlushRequest` の variant payload は `flush_id: u64 BE`、`scope: u8`、`lane_id: u32 BE`、`expected_acks: u32 BE` をこの順で持つ SHALL。`scope` は `0=Shutdown`、`1=BeforeDeathWatchNotification` とする。`scope` は flush outcome の意味を区別する metadata であり、`lane_id = 0` を control-only lane とみなす規則を含まない（MUST NOT）。

`FlushAck` の variant payload は `flush_id: u64 BE`、`lane_id: u32 BE`、`expected_acks: u32 BE` をこの順で持つ SHALL。

#### Scenario: ControlPdu::Heartbeat の subkind

- **WHEN** `ControlPdu::Heartbeat { .. }` を encode する
- **THEN** subkind byte は `0x00` である

#### Scenario: ControlPdu::Quarantine の subkind

- **WHEN** `ControlPdu::Quarantine { reason, .. }` を encode する
- **THEN** subkind byte は `0x01` で、`reason` は `Option<String>` の `Some` として encode される

#### Scenario: ControlPdu::Shutdown の subkind

- **WHEN** `ControlPdu::Shutdown { .. }` を encode する
- **THEN** subkind byte は `0x02` である

#### Scenario: ControlPdu::HeartbeatResponse の subkind

- **WHEN** `ControlPdu::HeartbeatResponse { uid, .. }` を encode する
- **THEN** subkind byte は `0x03` である
- **AND** `uid` は common fields の後ろに `u64 BE` として encode される

#### Scenario: FlushRequest の round-trip

- **WHEN** `ControlPdu::FlushRequest { authority, flush_id: 42, scope: Shutdown, lane_id: 1, expected_acks: 4 }` を encode して decode する
- **THEN** decode 後の PDU は同じ authority、flush id、scope、lane id、expected ack 数を保持する
- **AND** subkind byte は `0x04` である

#### Scenario: FlushAck の round-trip

- **WHEN** `ControlPdu::FlushAck { authority, flush_id: 42, lane_id: 1, expected_acks: 4 }` を encode して decode する
- **THEN** decode 後の PDU は同じ authority、flush id、lane id、expected ack 数を保持する
- **AND** subkind byte は `0x05` である

#### Scenario: unknown flush scope is rejected

- **WHEN** `FlushRequest` の scope byte が `0` または `1` 以外である control frame を decode する
- **THEN** decoder は `WireError::InvalidFormat` または同等の observable error を返す
- **AND** flush session state へ進めない
