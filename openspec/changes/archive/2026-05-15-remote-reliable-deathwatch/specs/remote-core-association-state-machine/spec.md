## ADDED Requirements

### Requirement: system message redelivery state

`Association` は system priority envelope の ACK/NACK redelivery state を所有する SHALL。対象は remote DeathWatch に必要な `Watch`、`Unwatch`、`DeathWatchNotification` 系 system message であり、user priority envelope はこの state に保持してはならない（MUST NOT）。

#### Scenario: system envelope receives sequence number

- **WHEN** `Association::enqueue` に system priority envelope が渡される
- **THEN** association は per-remote-node の単調増加 sequence number を割り当てる
- **AND** envelope は ACK を受けるまで resend window に保持される

#### Scenario: user envelope is not tracked by redelivery state

- **WHEN** `Association::enqueue` に user priority envelope が渡される
- **THEN** association は redelivery sequence number を割り当てない
- **AND** user envelope は ACK/NACK resend window に保持されない

#### Scenario: cumulative ack removes pending envelopes

- **GIVEN** sequence number `10`、`11`、`12` の system envelope が pending である
- **WHEN** `AckPdu { cumulative_ack: 11, .. }` を association に適用する
- **THEN** sequence number `10` と `11` は pending から削除される
- **AND** sequence number `12` は pending に残る

#### Scenario: nack bitmap selects missing envelopes for resend

- **GIVEN** sequence number `20` から `23` の system envelope が pending である
- **WHEN** `AckPdu { cumulative_ack: 20, nack_bitmap }` が sequence number `22` の欠落を示す
- **THEN** association は sequence number `22` の envelope を resend effect に含める
- **AND** ACK 済みの sequence number `20` は resend effect に含めない

### Requirement: inbound system sequence tracking

`Association` は inbound system priority envelope の sequence number を tracking し、受信済み範囲から cumulative ACK と NACK bitmap を生成する SHALL。重複 sequence number は actor-core へ二重配送してはならない（MUST NOT）。

#### Scenario: in-order system envelope advances ack

- **GIVEN** inbound cumulative ACK が `40` である
- **WHEN** sequence number `41` の system envelope を受信する
- **THEN** inbound cumulative ACK は `41` へ進む
- **AND** association は `AckPdu` 送信 effect を返す

#### Scenario: gap produces nack bitmap

- **GIVEN** inbound cumulative ACK が `50` である
- **WHEN** sequence number `52` の system envelope を受信する
- **THEN** inbound cumulative ACK は `50` のまま維持される
- **AND** association は sequence number `51` の欠落を示す NACK bitmap を持つ `AckPdu` 送信 effect を返す

#### Scenario: duplicate inbound system envelope is ignored

- **GIVEN** sequence number `60` の system envelope がすでに actor-core へ配送済みである
- **WHEN** 同じ sequence number `60` の system envelope を再受信する
- **THEN** association は actor-core delivery 対象として返さない
- **AND** ACK 状態は再送元が停止できる形で返される
