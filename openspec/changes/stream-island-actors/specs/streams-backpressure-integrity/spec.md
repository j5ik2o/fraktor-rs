## MODIFIED Requirements

### Requirement: island boundary は actor 分離後も backpressure と terminal signal を保持しなければならない

stream island boundary は、island が別 actor / 別 mailbox / 別 dispatcher で実行されても、要素、backpressure、completion、failure、cancellation を失ってはならない（MUST）。boundary full / empty を同期直実行の恒久 failure として扱ってはならない（MUST NOT）。

#### Scenario: boundary full は upstream island の pending として扱われる

- **GIVEN** upstream island actor が downstream boundary に要素を push している
- **AND** boundary capacity が満杯である
- **WHEN** upstream island actor が drive される
- **THEN** upstream island は要素を保持したまま pending になる
- **AND** 要素は drop されない
- **AND** downstream island が boundary を drain した後、upstream island は後続 drive で進捗できる

#### Scenario: boundary empty は downstream island の pending として扱われる

- **GIVEN** downstream island actor が boundary から pull している
- **AND** boundary が empty かつ open である
- **WHEN** downstream island actor が drive される
- **THEN** downstream island は failure ではなく pending として扱われる
- **AND** busy loop せず、次の drive command または boundary state transition まで待機する

#### Scenario: upstream completion は pending 要素の後に downstream completion になる

- **GIVEN** upstream island が completion に到達する
- **AND** boundary に未配送要素が残っている
- **WHEN** downstream island が boundary を drain する
- **THEN** downstream island は残り要素を受け取る
- **AND** 残り要素の後に completion を観測する

#### Scenario: upstream failure は downstream failure になる

- **GIVEN** upstream island actor が failure に到達する
- **WHEN** boundary が failure state に遷移する
- **THEN** downstream island actor は同じ materialized graph の failure として観測する
- **AND** downstream island は正常 completion として扱われない

#### Scenario: downstream cancellation は upstream island へ伝播する

- **GIVEN** downstream island actor が cancel される
- **WHEN** cancel が boundary に伝播する
- **THEN** upstream island actor は cancel または shutdown command を受け取る
- **AND** upstream island は新しい要素を boundary へ publish し続けない
- **AND** cancellation は boundary の data state だけで表現されず、materialized graph の control plane から upstream island actor へ配送される
