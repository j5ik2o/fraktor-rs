## ADDED Requirements

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
