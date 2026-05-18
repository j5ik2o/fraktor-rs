## MODIFIED Requirements

### Requirement: EnvelopePdu の binary レイアウト

`EnvelopePdu` は以下のフィールドを順に encode する SHALL。payload は actor-core `SerializedMessage` 相当の metadata と bytes に分解して保持し、raw application bytes だけを持つ layout であってはならない（MUST NOT）。

```
+---------------------+-------------------+
| field               | encoding          |
+---------------------+-------------------+
| frame header        | length(u32 BE)    |
|                     | + version(u8)     |
|                     | + kind(u8=0x01)   |
+---------------------+-------------------+
| recipient_path      | String            |
| sender_path         | Option<String>    |
| correlation_hi      | u64 BE            |
| correlation_lo      | u32 BE            |
| priority            | u8 (0=System,     |
|                     |    1=User)        |
| serializer_id       | u32 BE            |
| manifest            | Option<String>    |
| payload             | u32 length + bytes|
+---------------------+-------------------+
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

#### Scenario: serialized message metadata の round-trip

- **WHEN** `serializer_id = 7`、`manifest = Some("example.Manifest")`、`payload = b"hello"` を持つ `EnvelopePdu` を encode して decode する
- **THEN** decode 後の `EnvelopePdu` は同じ serializer id、manifest、payload bytes を保持する
- **AND** manifest は payload bytes に結合されず、独立した `Option<String>` として復元される

#### Scenario: manifest が None の場合

- **WHEN** `manifest = None` の `EnvelopePdu` を encode する
- **THEN** manifest field は `Option<String>` の `None` tag として encode される
- **AND** decode 後の manifest は `None` である
