## ADDED Requirements

### Requirement: SourceRef materialization follows Pekko direction

The system SHALL materialize a `SourceRef<T>` from a producer stream, and a serialized `SourceRef<T>` SHALL resolve back into a `SourceRef<T>` that the receiving side can consume as a source.

#### Scenario: producer materializes SourceRef

- **WHEN** a producer stream is run with the `StreamRefs.source_ref` equivalent
- **THEN** the materialized value is a `SourceRef<T>`
- **AND** the original producer stream is the source of elements for that ref

#### Scenario: serialized SourceRef resolves as SourceRef

- **WHEN** a serialized `SourceRef<T>` format is resolved
- **THEN** the resolver returns a `SourceRef<T>`
- **AND** the caller can consume it through `into_source` or the equivalent source materialization API

#### Scenario: SourceRef round-trip carries elements locally

- **GIVEN** a producer stream has materialized a `SourceRef<T>`
- **AND** the ref is backed by a materialized endpoint actor with a canonical actor path
- **WHEN** the same ActorSystem converts the ref to serialization format and resolves it back to `SourceRef<T>` through provider dispatch
- **AND** the resolved ref is consumed by a sink
- **THEN** all produced elements are observed in order
- **AND** normal completion is observed after the elements

### Requirement: SinkRef materialization follows Pekko direction

The system SHALL materialize a `SinkRef<T>` from a consumer sink, and a serialized `SinkRef<T>` SHALL resolve back into a `SinkRef<T>` that the receiving side can write to as a sink.

#### Scenario: consumer materializes SinkRef

- **WHEN** a consumer sink is run with the `StreamRefs.sink_ref` equivalent
- **THEN** the materialized value is a `SinkRef<T>`
- **AND** the original consumer sink is the destination for elements written through that ref

#### Scenario: serialized SinkRef resolves as SinkRef

- **WHEN** a serialized `SinkRef<T>` format is resolved
- **THEN** the resolver returns a `SinkRef<T>`
- **AND** the caller can produce into it through `into_sink` or the equivalent sink materialization API

#### Scenario: SinkRef round-trip carries elements locally

- **GIVEN** a consumer sink has materialized a `SinkRef<T>`
- **AND** the ref is backed by a materialized endpoint actor with a canonical actor path
- **WHEN** the same ActorSystem converts the ref to serialization format and resolves it back to `SinkRef<T>` through provider dispatch
- **AND** a producer stream writes to the resolved ref
- **THEN** the original consumer sink observes all elements in order
- **AND** normal completion is observed after the elements

### Requirement: actor path format is not the application workflow

The system SHALL treat canonical actor path strings as resolver and serializer internals. Application-level remote StreamRef workflows SHOULD pass typed `SourceRef<T>` and `SinkRef<T>` values in domain messages rather than exposing path strings as the primary user contract.

#### Scenario: domain message carries SourceRef

- **WHEN** a remote domain message contains a `SourceRef<T>` field
- **THEN** serialization support may encode the field as a canonical actor path string internally
- **AND** deserialization restores a `SourceRef<T>` for the receiving side

#### Scenario: domain message carries SinkRef

- **WHEN** a remote domain message contains a `SinkRef<T>` field
- **THEN** serialization support may encode the field as a canonical actor path string internally
- **AND** deserialization restores a `SinkRef<T>` for the receiving side

#### Scenario: reversed resolver workflow is not public

- **WHEN** public API examples, integration tests, or user-facing documentation describe StreamRef handoff
- **THEN** they do not present `SourceRef` serialization as resolving to `SinkRef`
- **AND** they do not present `SinkRef` serialization as resolving to `SourceRef`

### Requirement: actor-backed local materialization proof gates remote transport

The remote StreamRef implementation SHALL prove actor-backed endpoint materialization and resolver behavior before relying on remote transport integration. Local handoff-only refs, placeholder actor path strings, and in-process fake serialization maps MUST NOT satisfy this gate.

#### Scenario: local SourceRef proof is required before remote SourceRef test

- **WHEN** a two-ActorSystem `SourceRef` transport test is added
- **THEN** a non-ignored local `SourceRef` actor-backed endpoint proof already proves endpoint ownership, canonical path conversion, provider-dispatch resolve, element delivery, backpressure, and completion

#### Scenario: local SinkRef proof is required before remote SinkRef test

- **WHEN** a two-ActorSystem `SinkRef` transport test is added
- **THEN** a non-ignored local `SinkRef` actor-backed endpoint proof already proves endpoint ownership, canonical path conversion, provider-dispatch resolve, element delivery, backpressure, and completion

#### Scenario: endpoint state changes wake materialized stream

- **WHEN** handshake, demand, element, completion, failure, or cancellation reaches an endpoint actor or endpoint state holder
- **THEN** the paired materialized stream is woken or otherwise driven
- **AND** the stream does not wait indefinitely for another unrelated tick before observing the state change
