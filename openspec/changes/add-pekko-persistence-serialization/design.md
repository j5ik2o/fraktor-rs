## Context

Issue #529 covers the G-020 persistence serialization gap. Current fraktor-rs persistence has `PersistentRepr`, `Journal`, `Snapshot`, and `SnapshotStore`, but journal writes are represented as raw `Vec<PersistentRepr>` batches and snapshot / journal payloads remain erased `dyn Any` values without a durable serialization boundary.

Pekko keeps the plugin API typed around `PersistentRepr`, `AtomicWrite`, `SnapshotMetadata`, and `SelectedSnapshot`. Durable stores such as LevelDB and LocalSnapshotStore serialize at the store boundary by calling the common `SerializationExtension`; the persistence serializers then delegate domain payload and metadata encoding to the configured serializer registry. This change ports that responsibility model, not the JVM protobuf byte format.

## Goals / Non-Goals

**Goals:**
- Represent journal atomicity with a public `AtomicWrite` type.
- Make `Journal` and `JournalMessage::WriteMessages` use `AtomicWrite` units instead of unstructured `Vec<PersistentRepr>` batches.
- Add `MessageSerializer` and `SnapshotSerializer` in `persistence-core-kernel` that implement the actor serialization `Serializer` contract.
- Delegate nested payload and metadata serialization to the existing actor `SerializationRegistry` through `SerializationDelegator`.
- Automatically register persistence serializers and bindings when `PersistenceExtensionInstaller` installs the persistence extension.
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

5. Register serializers from the persistence installer.

   `PersistenceExtensionInstaller` SHALL ensure a `SerializationExtensionShared` exists via `default_serialization_extension_id()` and register persistence serializers plus type bindings against its registry. If a user installed a custom serialization extension first, put-if-absent extension semantics preserve that extension and persistence registration augments it.

6. Use fraktor internal wire structures.

   The persistence serializers SHALL encode enough metadata to round-trip `PersistentRepr`, `AtomicWrite`, and snapshot payload wrappers inside fraktor-rs. They do not need to match Pekko protobuf bytes. The nested payload representation SHOULD use existing `SerializedMessage` data (`serializer_id`, optional manifest, bytes) so all domain object evolution remains owned by the actor serialization registry.

7. Keep runtime event adapter registries out of the durable wire format.

   `PersistentRepr` contains an `EventAdapters` registry for runtime adaptation and an `adapter_type_id` for adapter resolution. The serializer SHALL preserve durable replay metadata, including `sender` and `adapter_type_id`, but SHALL NOT encode the `EventAdapters` registry internals because it contains runtime trait objects and configuration. Replay paths that require non-identity adapters must reattach the configured runtime registry around deserialized representations before adaptation.

## Risks / Trade-offs

- Breaking journal API changes → Update all in-tree `Journal` implementations, tests, and examples in the same change.
- Serializer registration collisions → Use fixed runtime serializer IDs outside existing built-in IDs and make registration idempotent with collision reporting consistent with existing built-ins.
- Nested payloads without registered serializers → Surface `SerializationError::NotSerializable` during persistence serialization; do not silently store erased payloads.
- Event adapter registry is runtime state → Preserve the adapter type id in bytes, and test that deserialization does not pretend to reconstruct the runtime adapter registry from durable data.
- Snapshot naming collision with existing `snapshot::Snapshot` → Keep the existing public snapshot container and add a clearly scoped serialization wrapper type in a `serialization` module if a separate wrapper is required.
- Byte-level incompatibility with Pekko → Document as a non-goal and keep field semantics close enough that a future protobuf-compatible serializer can be added without changing journal contracts again.
