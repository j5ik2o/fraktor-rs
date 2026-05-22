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
The journal write contract SHALL accept batches of `AtomicWrite` units, and each `AtomicWrite` SHALL be persisted atomically by a journal implementation.

#### Scenario: Journal actor writes atomic batches
- **WHEN** `JournalMessage::WriteMessages` is handled by `JournalActor`
- **THEN** the actor passes atomic write units to `Journal::write_messages`

#### Scenario: In-memory journal preserves atomic write entries
- **WHEN** `InMemoryJournal` stores a batch of atomic writes
- **THEN** replay returns the contained `PersistentRepr` entries in sequence order for the matching persistence id

### Requirement: Message serializer delegates persistent payloads
The persistence kernel SHALL provide a `MessageSerializer` that serializes and deserializes `PersistentRepr` and `AtomicWrite` while delegating each persistent payload and metadata value to the actor serialization registry and preserving durable persistent representation metadata.

#### Scenario: Persistent representation round trip
- **WHEN** `MessageSerializer` serializes a `PersistentRepr` whose payload serializer is registered
- **THEN** deserializing the bytes restores the persistence id, sequence number, manifest, writer uuid, timestamp, deleted flag, sender, adapter type id, payload, and metadata values

#### Scenario: Runtime event adapter registry is not durable data
- **WHEN** `MessageSerializer` serializes a `PersistentRepr` with a configured `EventAdapters` runtime registry
- **THEN** deserializing the bytes preserves the adapter type id needed for replay adapter resolution without encoding the runtime registry internals

#### Scenario: Atomic write round trip
- **WHEN** `MessageSerializer` serializes an `AtomicWrite`
- **THEN** deserializing the bytes restores an atomic write with the same contained persistent representations

#### Scenario: Unregistered payload rejected
- **WHEN** `MessageSerializer` serializes a persistent payload that has no matching actor serializer
- **THEN** serialization fails with a serialization error instead of storing an erased value

### Requirement: Snapshot serializer delegates snapshot data
The persistence kernel SHALL provide a `SnapshotSerializer` that serializes and deserializes snapshot payload wrappers while delegating snapshot data to the actor serialization registry.

#### Scenario: Snapshot payload round trip
- **WHEN** `SnapshotSerializer` serializes a snapshot payload wrapper whose data serializer is registered
- **THEN** deserializing the bytes restores the wrapped snapshot data

#### Scenario: Snapshot data serializer id is retained
- **WHEN** snapshot data is serialized
- **THEN** the encoded snapshot record contains the nested serializer id and manifest needed to deserialize through the actor serialization registry

### Requirement: Persistence installer registers serializers
The persistence extension installer SHALL automatically register persistence serializers and type bindings with the actor serialization extension during actor system bootstrap.

#### Scenario: Default serialization extension registration
- **WHEN** a system installs `PersistenceExtensionInstaller` without a preinstalled serialization extension
- **THEN** the installer registers the default serialization extension and then registers persistence serializers on it

#### Scenario: Existing serialization extension augmented
- **WHEN** a system installs a custom serialization extension before `PersistenceExtensionInstaller`
- **THEN** the persistence installer augments the existing serialization registry instead of replacing it

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
