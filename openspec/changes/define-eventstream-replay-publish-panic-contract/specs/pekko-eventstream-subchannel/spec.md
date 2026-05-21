## ADDED Requirements

### Requirement: subscribe replay は return 前に同期通知されなければならない

`EventStreamShared::subscribe_with_key(key, subscriber)` は、登録時点で key にマッチする buffered events の replay snapshot を確定し、`EventStreamSubscription` を返す前に対象 subscriber へ同期通知しなければならない（MUST）。この契約は subscribe 呼び出しと同じ thread の happens-before に限定され、subscribe 実行中に別 thread から実行される `publish()` との厳密な replay/live 順序は保証してはならない（MUST NOT）。

#### Scenario: subscribe return 後は replay が観測済み

- **GIVEN** `EventStreamShared` に Log event が buffered 済みである
- **WHEN** subscriber が `subscribe_with_key(ClassifierKey::Log, subscriber)` で登録され、その呼び出しが return する
- **THEN** subscriber は buffered Log event を既に受信している

#### Scenario: concurrent publish との replay/live 順序は未規定

- **GIVEN** subscriber が `subscribe_with_key` で登録中である
- **WHEN** 別 thread が同じ subscriber に match する live event を `publish()` する
- **THEN** implementation は replay callback と live callback の厳密な `buffered -> live` 順序を保証しない

### Requirement: publish は callback 完了を同期観測できなければならない

`EventStreamShared::publish(event)` は、panic が発生しない限り、配送対象 snapshot に含まれる subscriber callback を同期実行し、すべての対象 callback が完了してから return しなければならない（MUST）。配送対象 snapshot は `publish()` の event stream lock 区間で確定され、callback 実行中の subscribe / unsubscribe によって当該 publish の対象集合を変更してはならない（MUST NOT）。

#### Scenario: publish return 後は callback が完了済み

- **GIVEN** subscriber が `ClassifierKey::Log` で購読済みである
- **WHEN** caller が Log event を `publish()` し、その呼び出しが return する
- **THEN** subscriber callback はその Log event を観測済みである

#### Scenario: callback 中の購読変更は当該 publish の snapshot を変更しない

- **GIVEN** publish target snapshot が確定済みである
- **WHEN** subscriber callback が event stream の subscribe または unsubscribe を実行する
- **THEN** その変更は進行中の publish の配送対象集合を変更しない

### Requirement: subscriber panic は subscription lifecycle を自動変更してはならない

`EventStreamShared` は subscriber callback panic を catch / isolate してはならない（MUST NOT）。panic は `publish()` または replay 中の `subscribe_with_key()` 呼び出し元へ伝播しなければならず（MUST）、panic した subscriber の subscription は自動解除してはならない（MUST NOT）。panic 発生後に、panic した subscriber より後続の subscriber へ同じ event が配送されることは保証しない。

#### Scenario: publish 中の subscriber panic は呼び出し元へ伝播する

- **GIVEN** subscriber callback が Log event で panic する
- **WHEN** caller が Log event を `publish()` する
- **THEN** panic は `publish()` の呼び出し元へ伝播する

#### Scenario: panic した subscriber は自動解除されない

- **GIVEN** subscriber callback が最初の Log event で panic する
- **AND** caller がその panic を捕捉して処理を継続する
- **WHEN** caller が次の Log event を `publish()` する
- **THEN** 同じ subscriber は明示的に unsubscribe されていない限り次の Log event も受信する

#### Scenario: panic 後の後続 subscriber 配送は保証されない

- **GIVEN** panic する subscriber と別の subscriber が同じ key で購読済みである
- **WHEN** panic する subscriber の callback が `publish()` 中に panic する
- **THEN** implementation は同じ event を後続 subscriber へ配送することを保証しない
