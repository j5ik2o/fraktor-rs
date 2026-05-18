# Baseline Review

## Covered Tasks

- 1.1 Pekko StreamRef reference implementation review.
- 1.2 Current fraktor-rs StreamRef boundary review.
- 1.3 Crate boundary review for endpoint actor, resolver, and serializer.
- 1.4 Rejected designs that would break `stream-core-kernel` boundaries.

## Pekko StreamRef Semantics

Pekko `StreamRefs.sourceRef` materializes a `SourceRef[T]` from a local sink. The other side receives the same `SourceRef[T]` kind and consumes it as a source.

Pekko `StreamRefs.sinkRef` materializes a `SinkRef[T]` from a local source. The other side receives the same `SinkRef[T]` kind and writes to it as a sink.

`SourceRefImpl[T]` and `SinkRefImpl[T]` are thin actor-backed refs. Each stores the initial partner `ActorRef`. `StreamRefResolverImpl` serializes only those impls, encodes their actor path, and resolves the path back into the same ref kind through the actor provider.

`StreamRefSerializer` covers both ref payloads and protocol payloads. It serializes `SourceRefImpl`, `SinkRefImpl`, `OnSubscribeHandshake`, `CumulativeDemand`, `SequencedOnNext`, `RemoteStreamCompleted`, `RemoteStreamFailure`, and `Ack`.

The endpoint actor is not an independent transport. It is the stage actor owned by the materialized stream stage. `StreamRefsMaster` only provides stable stage actor names. The stage logic owns subscription timeout, one-shot partner pairing, partner watch/unwatch, cumulative demand, sequence validation, terminal ordering, and delayed shutdown around partner termination.

## Current fraktor-rs Boundary

Current `stream-core-kernel` already matches the public direction:

- `StreamRefs::source_ref<T>() -> Sink<T, SourceRef<T>>`
- `StreamRefs::sink_ref<T>() -> Source<T, SinkRef<T>>`
- `SourceRef<T>::into_source(self) -> Source<T, StreamNotUsed>`
- `SinkRef<T>::into_sink(self) -> Sink<T, StreamNotUsed>`

However, current refs are local handoff wrappers only:

- `SourceRef<T>` stores `StreamRefHandoff<T>`.
- `SinkRef<T>` stores `StreamRefHandoff<T>`.
- `StreamRefHandoff<T>` stores local protocol messages, subscription flag, failure state, buffer capacity, and local sequence counters.

They do not store endpoint `ActorRef`, canonical actor path, materialized stream ownership, partner watch state, endpoint shutdown hook, or a wake / drive bridge from actor messages back to the materialized stream.

## Crate Boundary Decision

`stream-core-kernel` may keep the typed `SourceRef<T>` / `SinkRef<T>` wrappers, protocol model, local handoff backend, sequence validation, demand validation, terminal ordering, and error mapping. It must remain `no_std` and must not depend on `remote-core`, `remote-adaptor-std`, tokio, TCP, or std-only resources.

`stream-adaptor-std` is the best current home for std actor-backed StreamRef endpoint wiring. It already depends on `actor-core-kernel` and `stream-core-kernel`, so it can own endpoint actor construction, materialized resource ownership, wake / drive integration, and resolver helpers that call `ActorSystem::resolve_actor_ref`.

`remote-adaptor-std` should stay a reusable remote actor provider, not a StreamRef implementation crate. StreamRef resolver code should use the `ActorSystem` provider surface; when `remote-adaptor-std` has installed `StdRemoteActorRefProvider`, remote paths naturally resolve through existing provider dispatch.

`actor-core-kernel` already provides the required neutral infrastructure: `ActorRef::canonical_path`, `ActorSystem::resolve_actor_ref`, actor provider dispatch by scheme, watch/unwatch system messages, and `SerializationRegistry` / manifest routing.

If `stream-adaptor-std` plus runtime-installed remote provider is not enough for final serializer wiring, the fallback is a small stream/remote integration crate. That is preferable to making `stream-core-kernel` remote-aware or making `remote-adaptor-std` own stream semantics.

## Rejected Designs

- Do not serialize current local `StreamRefHandoff<T>` through an in-process map. That would not prove actor-backed materialization.
- Do not treat actor path strings as the application workflow. They remain resolver / serializer internals.
- Do not add StreamRef-specific methods or wire frames to `RemoteTransport`.
- Do not put endpoint actor spawning, tokio tasks, TCP, or remote-provider-specific code in `stream-core-kernel`.
- Do not make `remote-adaptor-std` construct stream internals directly unless an explicit integration crate decision is made.
