## Context

Issue #529 covers the G-020 persistence serialization gap. Current fraktor-rs persistence has `PersistentRepr`, `Journal`, `Snapshot`, and `SnapshotStore`, but journal writes are represented as raw `Vec<PersistentRepr>` batches and snapshot / journal payloads remain erased `dyn Any` values without a durable serialization boundary.

Pekko keeps the plugin API typed around `PersistentRepr`, `AtomicWrite`, `SnapshotMetadata`, and `SelectedSnapshot`. Durable stores such as LevelDB and LocalSnapshotStore serialize at the store boundary by calling the common `SerializationExtension`; the persistence serializers then delegate domain payload and metadata encoding to the configured serializer registry. This change ports that responsibility model, not the JVM protobuf byte format.

## Goals / Non-Goals

**Goals:**
- Represent journal atomicity with a public `AtomicWrite` type.
- Make `Journal` and `JournalMessage::WriteMessages` use `AtomicWrite` units instead of unstructured `Vec<PersistentRepr>` batches.
- Add `MessageSerializer` and `SnapshotSerializer` in `persistence-core-kernel` that implement the actor serialization `Serializer` contract.
- Delegate nested payload and metadata serialization to the existing actor `SerializationRegistry` through `SerializationDelegator`.
- Automatically contribute persistence serializers and bindings to the actor serialization extension during actor system bootstrap.
- Preserve no_std core constraints by using `alloc` and existing actor-core serialization primitives.

**Non-Goals:**
- Pekko protobuf byte-level compatibility for `MessageFormats.proto`.
- A disk-backed journal, local snapshot store, or `persistence-adaptor-std` crate.
- JVM migration behavior such as Akka/Pekko manifest auto-migration.
- Backward-compatible support for the pre-change `Vec<PersistentRepr>` write API.

## Decisions

1. Add `AtomicWrite` as a public persistence type.

   `AtomicWrite` SHALL contain a non-empty `Vec<PersistentRepr>` and SHALL validate that all entries use the same persistence id. It exposes `persistence_id`, `lowest_sequence_nr`, `highest_sequence_nr`, `size`, and `payload` accessors. This follows Pekko `AtomicWrite` and makes `persist_all` all-or-none semantics explicit. The alternative, keeping `Vec<PersistentRepr>`, leaves the atomicity boundary implicit and makes a Pekko-compatible `MessageSerializer` incomplete.

2. Change journal write contracts to `AtomicWrite`.

   `Journal::write_messages` SHALL receive `&[AtomicWrite]`, and `JournalMessage::WriteMessages` SHALL carry `Vec<AtomicWrite>`. `JournalActor` responses can still emit per-`PersistentRepr` success/failure messages by iterating atomic writes. This is intentionally breaking because the project is pre-release and the old contract hides a domain invariant.

3. Implement persistence serializers in `persistence-core-kernel`.

   `persistence-core-kernel` already depends on `actor-core-kernel`, so the serializers can implement `fraktor_actor_core_kernel_rs::serialization::Serializer` without creating a dependency cycle. Placing them in actor-core would invert the domain dependency and make actor-core know persistence types.

4. Use weak registry delegation for nested payloads.

   `MessageSerializer` and `SnapshotSerializer` SHALL hold a `WeakShared<SerializationRegistry>`, matching the existing `MiscMessageSerializer` pattern. This avoids reference cycles because the registry stores serializer instances and the serializers need the registry for nested payload encode/decode.

5. Register serializers through an order-independent bootstrap contribution.

   `PersistenceExtensionInstaller` SHALL NOT create the default `SerializationExtension` as a side effect that can block a later custom `SerializationExtensionInstaller`. Instead, persistence SHALL contribute serializers, manifest routes, and type bindings to the final serialization setup before the serialization extension is instantiated, or augment an already-instantiated serialization extension only when no later custom serialization setup can be shadowed. Installing persistence before or after custom serialization setup must produce the same registry contents.

6. Fail fast on persistence serializer registration collisions.

   Persistence serializer IDs are runtime contracts. If an ID required by `MessageSerializer` or `SnapshotSerializer` is already occupied by a different serializer, bootstrap SHALL fail before binding persistence types. Re-registering the same persistence serializer set is idempotent. Silent skip is not allowed because it can bind `PersistentRepr`, `AtomicWrite`, or snapshot wrappers to an unintended serializer.

7. Use fraktor internal wire structures.

   The persistence serializers SHALL encode enough metadata to round-trip `PersistentRepr`, `AtomicWrite`, and snapshot payload wrappers inside fraktor-rs. They do not need to match Pekko protobuf bytes. The nested payload representation SHOULD use existing `SerializedMessage` data (`serializer_id`, optional manifest, bytes) so all domain object evolution remains owned by the actor serialization registry.

8. Limit nested payload round-trip guarantees to manifest-resolvable serializers.

   `MessageSerializer` and `SnapshotSerializer` deserialize nested objects from durable `SerializedMessage` data, not from the caller's concrete Rust type hint. A nested serializer is eligible for the round-trip guarantee only when it can reconstruct the object from serializer id plus encoded manifest. Serializers that require an external type hint must fail with a serialization error instead of producing a partially typed or erased value.

9. Keep runtime event adapter registries out of the durable wire format.

   `PersistentRepr` contains an `EventAdapters` registry for runtime adaptation and an `adapter_type_id` for adapter resolution. The serializer SHALL preserve durable replay metadata, including `sender` and `adapter_type_id`, but SHALL NOT encode the `EventAdapters` registry internals because it contains runtime trait objects and configuration. Replay paths that require non-identity adapters must reattach the configured runtime registry around deserialized representations before adaptation.

## Risks / Trade-offs

- Breaking journal API changes → Update all in-tree `Journal` implementations, tests, and examples in the same change.
- Serializer registration collisions → Use fixed runtime serializer IDs outside existing built-in IDs, make duplicate persistence registration idempotent, and fail fast when a different serializer occupies the required ID.
- Installer ordering → Compose persistence serializer setup with custom serialization setup before extension instantiation so persistence does not accidentally install the default serialization extension first.
- Nested payloads without registered serializers → Surface `SerializationError::NotSerializable` during persistence serialization; do not silently store erased payloads.
- Nested payloads without manifest-resolvable deserialization → Surface a serialization error; do not overpromise round-trip for serializers that require a concrete type hint during decode.
- Event adapter registry is runtime state → Preserve the adapter type id in bytes, and test that deserialization does not pretend to reconstruct the runtime adapter registry from durable data.
- Snapshot naming collision with existing `snapshot::Snapshot` → Keep the existing public snapshot container and add a clearly scoped serialization wrapper type in a `serialization` module if a separate wrapper is required.
- Byte-level incompatibility with Pekko → Document as a non-goal and keep field semantics close enough that a future protobuf-compatible serializer can be added without changing journal contracts again.
