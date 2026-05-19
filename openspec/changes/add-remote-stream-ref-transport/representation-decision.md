# StreamRef Representation Decision

## Covered Tasks

- 2.1 Decide `SourceRef<T>` / `SinkRef<T>` internal representation for local handoff and remote actor-backed refs.
- 2.2 Decide minimum data held by remote actor-backed refs.
- 2.3 Define local-only serialization failure.
- 2.4 Fix resolver direction names.
- 2.5 Keep application examples on typed ref payloads, not actor path strings.

## Decision

`SourceRef<T>` and `SinkRef<T>` should become typed wrappers over an internal backend:

- local handoff backend,
- actor-backed endpoint backend.

The local handoff backend is the current `StreamRefHandoff<T>` path and must keep current local behavior unchanged.

The actor-backed endpoint backend must hold an endpoint actor reference, not a copied local handoff. The endpoint actor reference is the serializable identity of a remote-capable ref. A canonical actor path is derived from the actor reference when serialization is requested.

The backend must be private implementation detail of `stream-core-kernel`. Application code still sees typed `SourceRef<T>` and `SinkRef<T>`.

## Minimum Actor-Backed Data

The actor-backed backend needs:

- endpoint `ActorRef`,
- canonical actor path availability through `ActorRef::canonical_path`,
- ref kind through the owning wrapper type (`SourceRef<T>` or `SinkRef<T>`),
- type marker through `T`,
- one-shot pairing state in the endpoint actor, not in the wrapper,
- materialized resource owner in the endpoint actor / materializer resource, not in the wrapper.

The wrapper should not own remote transport, TCP handles, serializer registry, tokio tasks, or partner watch state directly.

## Serialization Rule

Only actor-backed refs are serializable as remote StreamRefs.

When resolver support receives a local handoff-only `SourceRef<T>` or `SinkRef<T>`, it must return an explicit unsupported-local-ref failure. It must not create a fake actor path, store the handoff in a process-local map, or treat the local handoff as proof of remote-capable materialization.

When resolver support receives an actor-backed ref whose endpoint lacks a canonical path, it must return an explicit missing-endpoint-path failure.

## Resolver Direction

Names and tests must keep the Pekko direction:

- serialized `SourceRef` resolves to `SourceRef<T>`,
- serialized `SinkRef` resolves to `SinkRef<T>`.

Internal code may create partner endpoints, but public API names, test names, and serializer helper names must not present `SourceRef` as resolving to `SinkRef` or `SinkRef` as resolving to `SourceRef`.

## Application Workflow

Examples and integration tests should use typed domain payloads:

- a message containing `SourceRef<T>` that the receiver consumes as a source,
- a message containing `SinkRef<T>` that the receiver writes to as a sink.

Actor path string conversion belongs only in resolver / serializer tests and implementation helpers.

## Current Implementation Boundary

`SourceRef<T>` and `SinkRef<T>` now wrap a private backend:

- local backend: `StreamRefHandoff<T>` plus the materialized endpoint slot;
- actor-backed backend: endpoint actor slot only.

`StreamRefs::source_ref<T>()` wires the `SourceRef<T>` slot into `StreamRefSinkLogic`, and `StreamRefs::sink_ref<T>()` wires the `SinkRef<T>` slot into `StreamRefSourceLogic`. Materialization installs a real `StageActor` endpoint into each slot.

The lower-level proofs convert materialized `SourceRef<T>` and `SinkRef<T>` values to their endpoint actors' canonical paths, so tasks 4.1 and 4.2 are no longer satisfied by fake path strings or by serializing the local handoff.

`StreamRefResolver` resolves serialized `SourceRef` and `SinkRef` paths through `ActorSystem::resolve_actor_ref` and returns the same ref kind backed by the resolved endpoint actor. It does not allocate a new local handoff for resolved refs.

The local loopback proof sends messages through the resolved endpoint `ActorRef` and observes them on the original `StageActor`, so the resolver proof goes through provider dispatch and normal local actor delivery rather than direct transport construction. Failure tests cover local refs without endpoint actors, invalid path formats, and valid paths whose endpoint actor cannot be resolved.

Protocol element delivery is still pending. Actor-backed `into_source` and `into_sink` currently fail explicitly instead of pretending the resolved endpoint is a local handoff. That is the next boundary for loopback actor delivery and protocol message handling.
