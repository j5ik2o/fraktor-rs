## ADDED Requirements

### Requirement: Atomic write boundary
The persistence kernel SHALL expose an `AtomicWrite` type that represents one all-or-none journal write unit containing one or more `PersistentRepr` entries for a single persistence id.

#### Scenario: Valid atomic write
- **WHEN** an `AtomicWrite` is created from non-empty persistent representations with the same persistence id
- **THEN** the atomic write exposes the persistence id, lowest sequence number, highest sequence number, size, and payload entries

#### Scenario: Empty atomic write rejected
- **WHEN** an `AtomicWrite` is created from an empty payload
- **THEN** creation fails without producing an atomic write

#### Scenario: Mixed persistence ids rejected
- **WHEN** an `AtomicWrite` is created from persistent representations with different persistence ids
- **THEN** creation fails without producing an atomic write

### Requirement: Journal writes use atomic writes
The journal write contract SHALL accept batches of `AtomicWrite` units, and each `AtomicWrite` SHALL be persisted atomically by a journal implementation or rejected without partial persistence when the backend cannot guarantee atomicity.

#### Scenario: Journal actor writes atomic batches
- **WHEN** `JournalMessage::WriteMessages` is handled by `JournalActor`
- **THEN** the actor passes atomic write units to `Journal::write_messages`

#### Scenario: In-memory journal preserves atomic write entries
- **WHEN** `InMemoryJournal` stores a batch of atomic writes
- **THEN** replay returns the contained `PersistentRepr` entries in sequence order for the matching persistence id

#### Scenario: Unsupported multi-event atomic write rejected
- **WHEN** a journal backend receives a multi-entry `AtomicWrite` but cannot guarantee all-or-none persistence for that backend
- **THEN** the journal returns a deterministic unsupported-operation error without persisting a prefix of the atomic write

### Requirement: Message serializer delegates persistent payloads
The persistence kernel SHALL provide a `MessageSerializer` that serializes and deserializes `PersistentRepr` and `AtomicWrite` while delegating each persistent payload and metadata value to the actor serialization registry and preserving durable persistent representation metadata.

#### Scenario: Persistent representation round trip
- **WHEN** `MessageSerializer` serializes a `PersistentRepr` whose payload and metadata serializers can deserialize from serializer id plus encoded manifest
- **THEN** deserializing the bytes restores the persistence id, sequence number, manifest, writer uuid, timestamp, deleted flag, payload, and metadata values

#### Scenario: Runtime replay context is not durable data
- **WHEN** `MessageSerializer` serializes a journal `PersistentRepr` that was created from actor runtime context
- **THEN** deserializing the bytes does not restore sender, the `EventAdapters` runtime registry, or a Rust `TypeId` adapter key from durable bytes

#### Scenario: Atomic write round trip
- **WHEN** `MessageSerializer` serializes an `AtomicWrite`
- **THEN** deserializing the bytes restores an atomic write with the same contained persistent representations

#### Scenario: Unregistered payload rejected
- **WHEN** `MessageSerializer` serializes a persistent payload that has no matching actor serializer
- **THEN** serialization fails with a serialization error instead of storing an erased value

### Requirement: Snapshot serializer delegates snapshot data
The persistence kernel SHALL provide a `SnapshotSerializer` that serializes and deserializes snapshot payload wrappers while delegating snapshot data to the actor serialization registry.

#### Scenario: Snapshot payload round trip
- **WHEN** `SnapshotSerializer` serializes a snapshot payload wrapper whose data serializer can deserialize from serializer id plus encoded manifest
- **THEN** deserializing the bytes restores the wrapped snapshot data

#### Scenario: Snapshot data serializer id is retained
- **WHEN** snapshot data is serialized
- **THEN** the encoded snapshot record contains the nested serializer id and manifest needed to deserialize through the actor serialization registry

### Requirement: Persistence installer registers serializers
The persistence extension installer SHALL automatically register persistence serializers and type bindings with the actor serialization extension during actor system bootstrap without making custom serialization setup depend on installer insertion order.

#### Scenario: Default serialization extension registration
- **WHEN** a system installs `PersistenceExtensionInstaller` without custom serialization setup
- **THEN** actor system bootstrap creates a serialization extension containing default actor serializers and persistence serializers

#### Scenario: Custom serialization setup is order independent
- **WHEN** a system installs both custom serialization setup and `PersistenceExtensionInstaller` in either insertion order
- **THEN** actor system bootstrap creates one serialization extension containing the custom serializers, default actor serializers, and persistence serializers

#### Scenario: Persistence serializer id collision rejected
- **WHEN** persistence serializer registration finds a serializer id already occupied by a different serializer
- **THEN** actor system bootstrap fails before registering persistence bindings that would resolve through the occupied serializer id

#### Scenario: Persistence type binding collision rejected
- **WHEN** persistence serializer registration finds `PersistentRepr`, `AtomicWrite`, or snapshot payload wrapper already bound to a different serializer id
- **THEN** actor system bootstrap fails before overwriting the existing type binding

#### Scenario: Persistent messages serialize after persistence install
- **WHEN** the persistence extension has been installed
- **THEN** `PersistentRepr`, `AtomicWrite`, and snapshot payload wrappers can be resolved by the actor serialization registry

### Requirement: Pekko responsibility compatibility
The persistence serialization layer SHALL follow Pekko responsibility boundaries for `PersistentRepr`, `AtomicWrite`, message payload delegation, and snapshot payload delegation without requiring Pekko protobuf byte-level compatibility.

#### Scenario: Durable store boundary
- **WHEN** a future durable journal or snapshot store needs bytes
- **THEN** it can call the actor serialization extension on persistence objects and rely on persistence serializers to delegate nested payloads

#### Scenario: Pekko bytes are not required
- **WHEN** bytes produced by Pekko JVM serializers are provided directly to fraktor-rs persistence serializers
- **THEN** compatibility is not guaranteed by this capability
