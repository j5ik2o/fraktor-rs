## MODIFIED Requirements

### Requirement: ClassifierKey は EventStreamEvent の全 variant に対応しなければならない

`ClassifierKey` enum は `EventStreamEvent` の全 15 variant に対応する具象 key と、全 variant を束ねる `All` を提供しなければならない（MUST）。`for_event(&EventStreamEvent)` は具象 variant のみ返し、`All` を返してはならない。

#### Scenario: 15 variant が全て具象 key を持つ

- **WHEN** `ClassifierKey::for_event(&event)` を各 `EventStreamEvent` variant で呼び出す
- **THEN** `Lifecycle / Log / DeadLetter / Extension / Mailbox / MailboxPressure / UnhandledMessage / AdapterFailure / Serialization / RemoteAuthority / RemotingBackpressure / RemotingLifecycle / AddressTerminated / SchedulerTick / TickDriver` のいずれかが返る
- **AND** `All` は返らない

#### Scenario: 主要 5 variant の対応関係

- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::Lifecycle(_))` を呼ぶ
- **THEN** `ClassifierKey::Lifecycle` が返る
- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::Log(_))` を呼ぶ
- **THEN** `ClassifierKey::Log` が返る
- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::DeadLetter(_))` を呼ぶ
- **THEN** `ClassifierKey::DeadLetter` が返る
- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::AddressTerminated(_))` を呼ぶ
- **THEN** `ClassifierKey::AddressTerminated` が返る
- **WHEN** `ClassifierKey::for_event(&EventStreamEvent::Extension { .. })` を呼ぶ
- **THEN** `ClassifierKey::Extension` が返る
