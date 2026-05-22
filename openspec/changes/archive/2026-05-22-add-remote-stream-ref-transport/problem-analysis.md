# Remote StreamRef Change Problem Analysis

## Summary

This change was under-specified. It tried to move from the existing local `SourceRef<T>` / `SinkRef<T>` handoff directly into resolver, serializer, and remote transport tasks without first deciding what a remote-capable StreamRef endpoint is.

That ordering is wrong. A resolver can only serialize a materialized ref if the ref is backed by an endpoint actor with a canonical actor path. The current `stream-core-kernel` refs are local handoff wrappers and do not carry that endpoint actor.

## What Is Wrong With The Current Shape

The previous task flow implied this sequence:

1. prove local resolver round-trip,
2. add resolver support,
3. wire serializer and remote actor delivery,
4. add endpoint actor behavior.

This is backwards. The endpoint actor representation and ownership model must come before resolver support. Otherwise the "serialization format" is either fake local state or a path string that does not correspond to a materialized endpoint actor.

## Current Implementation Boundary

`stream-core-kernel` currently models StreamRef as local handoff:

- `SourceRef<T>` owns `StreamRefHandoff<T>` and converts to a local `Source<T, StreamNotUsed>`.
- `SinkRef<T>` owns `StreamRefHandoff<T>` and converts to a local `Sink<T, StreamNotUsed>`.
- `StreamRefHandoff<T>` owns local protocol sequencing, completion ordering, failure propagation, buffer capacity, and subscription state.

That is valid for local stream behavior, but it is not a remote-capable endpoint representation.

The current refs do not carry:

- endpoint `ActorRef`,
- canonical actor path,
- materialized endpoint actor ownership,
- partner watch state,
- endpoint wake / drive integration,
- deterministic endpoint shutdown hooks.

## Bad Designs To Avoid

Do not make a fake local serialization format pass tests. It proves only that an in-process map can store a `StreamRefHandoff<T>`, not that a materialized StreamRef can cross an ActorSystem boundary.

Do not move actor / remote / tokio concerns into `stream-core-kernel`. That breaks the no_std and remote-independent boundary.

Do not expose actor path strings as the application workflow. Pekko-compatible application code should pass typed `SourceRef<T>` / `SinkRef<T>` payloads; path strings are resolver / serializer internals.

Do not implement serializer support before endpoint ownership is clear. Serialization of a ref must encode a real endpoint actor path, not a placeholder.

## Existing Infrastructure That Can Be Reused

The generic remote actor path is not the main gap:

- `ActorSystem::resolve_actor_ref` dispatches by actor path scheme through the installed provider.
- `StdRemoteActorRefProvider` handles local loopback and remote `fraktor.tcp` paths.
- `RemoteActorRefSender` sends normal `RemoteEvent::OutboundEnqueued` actor envelopes.
- `StdRemoteWatchHook` connects actor watch / unwatch and death-watch notifications to remote paths.
- `SerializationRegistry` supports runtime serializer and manifest route registration.

The missing piece is the StreamRef endpoint representation and ownership model.

## Required Reframe

This change should be reframed as:

> define and implement remote-capable StreamRef endpoint semantics, then attach resolver / serializer / remote actor delivery to that endpoint model.

The correct order is:

1. decide how `SourceRef<T>` / `SinkRef<T>` represent local handoff vs actor-backed endpoint refs,
2. define endpoint actor ownership as a materialized stream resource,
3. define wake / drive behavior for endpoint state changes,
4. serialize only actor-backed endpoint refs to canonical actor paths,
5. resolve serialized refs through ActorSystem provider dispatch,
6. carry protocol messages through normal remote actor envelopes,
7. verify two-ActorSystem typed payload workflows.

## Required Invariants

- `SourceRef` resolves to `SourceRef`; `SinkRef` resolves to `SinkRef`.
- Local-only refs must not serialize successfully as remote refs.
- `stream-core-kernel` remains no_std and remote-independent.
- Endpoint actor state changes must wake or drive the materialized stream.
- Remote termination before accepted protocol completion is stream failure, not normal completion.
