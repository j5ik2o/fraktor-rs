## ADDED Requirements

### Requirement: StreamRef resolver serializes refs as canonical actor paths

The system SHALL provide a StreamRef resolver contract that converts `SourceRef` and `SinkRef` backed by remote-capable endpoints to canonical actor path strings, and resolves those strings back into refs through the actor-ref provider surface.

#### Scenario: SourceRef is converted to serialization format

- **WHEN** a remote-capable `SourceRef` is passed to the StreamRef resolver
- **THEN** the resolver returns the canonical actor path string of the SourceRef endpoint actor
- **AND** the string includes the remote authority needed by another ActorSystem to resolve it

#### Scenario: SinkRef is converted to serialization format

- **WHEN** a remote-capable `SinkRef` is passed to the StreamRef resolver
- **THEN** the resolver returns the canonical actor path string of the SinkRef endpoint actor
- **AND** the string includes the remote authority needed by another ActorSystem to resolve it

#### Scenario: Serialized SourceRef is resolved through provider dispatch

- **WHEN** another ActorSystem resolves a serialized SourceRef string
- **THEN** the resolver uses the actor-ref provider surface to materialize the remote endpoint ActorRef
- **AND** it does not parse the path into a transport connection by itself

#### Scenario: Serialized SinkRef is resolved through provider dispatch

- **WHEN** another ActorSystem resolves a serialized SinkRef string
- **THEN** the resolver uses the actor-ref provider surface to materialize the remote endpoint ActorRef
- **AND** it does not parse the path into a transport connection by itself

### Requirement: stream-core remains independent from remote and std

The remote StreamRef implementation SHALL keep `stream-core-kernel` independent from `remote-core`, `remote-adaptor-std`, tokio, TCP, and std-only ActorSystem resources. Core SHALL own protocol semantics, settings, sequence validation, local handoff, and stream errors only.

#### Scenario: stream-core has no remote dependency

- **WHEN** `stream-core-kernel` Cargo dependencies are inspected
- **THEN** they do not include `fraktor-remote-core-rs` or `fraktor-remote-adaptor-std-rs`
- **AND** remote endpoint actor wiring is not implemented inside `stream-core-kernel`

#### Scenario: std integration owns remote endpoint actor wiring

- **WHEN** remote StreamRef endpoint actors, resolver installation, or serializer registration are implemented
- **THEN** those implementations live in a std adaptor or integration layer
- **AND** they may depend on actor / remote std runtime resources without leaking those dependencies into stream-core

### Requirement: StreamRef protocol messages are delivered as remote actor payloads

Remote StreamRef communication SHALL deliver StreamRef protocol messages through normal remote actor envelopes. The implementation MUST NOT add a StreamRef-specific `RemoteTransport` method or wire frame for this change.

#### Scenario: demand is sent as a protocol payload

- **WHEN** the receiving side requests more elements from a remote StreamRef partner
- **THEN** cumulative demand is sent to the partner endpoint actor as a serialized StreamRef protocol payload

#### Scenario: element is sent as a sequenced protocol payload

- **WHEN** the producing side has demand and an element to deliver
- **THEN** the element is sent as a sequenced StreamRef protocol payload through remote actor delivery

#### Scenario: transport port is not extended for StreamRef

- **WHEN** `RemoteTransport` is inspected after this change
- **THEN** it does not contain StreamRef-specific methods
- **AND** StreamRef delivery uses the existing outbound envelope path

### Requirement: StreamRef endpoints enforce one-shot partner pairing

A remote StreamRef endpoint SHALL pair with exactly one partner actor. After the first valid subscription handshake, messages from any different actor MUST fail the stream as an invalid partner condition.

#### Scenario: first handshake fixes the partner

- **WHEN** a remote StreamRef endpoint receives its first valid subscription handshake
- **THEN** it records the sender actor as the only valid partner for the lifetime of that ref

#### Scenario: second partner is rejected

- **WHEN** a different actor sends demand, element, completion, failure, or handshake messages to an already paired StreamRef endpoint
- **THEN** the endpoint fails the stream with an invalid partner error
- **AND** it does not accept the second actor as a new partner

#### Scenario: double materialization is rejected

- **WHEN** the same serialized StreamRef is resolved and materialized more than once as an active remote connection
- **THEN** only one partner pairing can succeed
- **AND** the later materialization fails rather than sharing the same ref

### Requirement: remote termination is visible as StreamRef failure

Remote partner actor termination, remote address termination, and transport connection loss SHALL be observed as StreamRef failures. They MUST NOT be converted to normal stream completion.

#### Scenario: partner DeathWatch fails the stream

- **WHEN** the endpoint actor receives a DeathWatch notification for its paired partner before normal protocol completion
- **THEN** the materialized stream fails with a remote StreamRef actor terminated error

#### Scenario: address termination fails the stream

- **WHEN** the remote address that owns the paired endpoint is published as terminated before normal protocol completion
- **THEN** the materialized stream fails with a remote-address termination stream error
- **AND** the failure context identifies the terminated remote address

#### Scenario: transport connection loss fails the stream

- **WHEN** the remote actor delivery path reports connection loss for the paired endpoint before normal protocol completion
- **THEN** the materialized stream fails with a transport-related stream error
- **AND** the failure is not reported as normal completion or as an invalid partner error

#### Scenario: normal completion remains protocol-driven

- **WHEN** the partner sends the sequenced remote stream completed message
- **THEN** the stream completes normally only after sequence validation succeeds
- **AND** no remote termination signal has already failed the stream

### Requirement: sequence and failure validation is preserved across remote boundary

Remote StreamRef endpoints SHALL validate sequence numbers, demand values, protocol failures, and partner identity with the same strictness as local StreamRef handoff.

#### Scenario: out-of-order element fails the stream

- **WHEN** a remote StreamRef endpoint receives `SequencedOnNext` with a sequence number different from the expected sequence number
- **THEN** the endpoint fails the stream with an invalid sequence number error

#### Scenario: invalid demand fails the stream

- **WHEN** a remote StreamRef endpoint receives demand with a zero or otherwise invalid demand count
- **THEN** the endpoint fails the stream with an invalid demand error

#### Scenario: remote failure message fails the stream

- **WHEN** a remote StreamRef endpoint receives a remote stream failure protocol message from its partner
- **THEN** the local materialized stream fails with the received failure context
