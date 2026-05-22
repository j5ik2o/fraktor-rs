# Endpoint Ownership And Wake Contract

## Covered Tasks

- 3.1 Define ownership for materialized `SourceRef<T>` endpoint actors.
- 3.2 Define ownership for materialized `SinkRef<T>` endpoint actors.
- 3.3 Define the wake / drive path after endpoint state changes.
- 3.4 Define deterministic endpoint actor shutdown and observable watch/shutdown cleanup failures.
- 3.5 Define one-shot partner pairing, double materialization failure, and non-partner message failure in endpoint state.
- 4.1 Materialize actor-backed `SourceRef<T>` endpoint identity and prove canonical actor path conversion.
- 4.2 Materialize actor-backed `SinkRef<T>` endpoint identity and prove canonical actor path conversion.
- 4.3 Resolve serialized `SourceRef` and `SinkRef` formats through `ActorSystem` provider dispatch without creating a fake local handoff.
- 4.4 Prove loopback authority resolves to local actor delivery without constructing a transport connection.
- 4.5 Prove local-only refs, missing endpoint actors, and invalid path formats fail explicitly.

## SourceRef Endpoint Ownership

When a producer stream is materialized through `StreamRefs::source_ref<T>()`, the materialized stream owns a source-side endpoint actor. The returned `SourceRef<T>` points at that endpoint actor.

The endpoint actor is a materialized resource of the producer stream. It is not a global resolver entry and not an independent remote transport. Its lifetime follows the materialized stream: completion, cancellation, failure, or materializer rollback must release the endpoint.

The endpoint actor is responsible for receiving partner handshake and cumulative demand, then driving the producer stream only when demand is available.

## SinkRef Endpoint Ownership

When a consumer sink is materialized through `StreamRefs::sink_ref<T>()`, the materialized stream owns a sink-side endpoint actor. The returned `SinkRef<T>` points at that endpoint actor.

The endpoint actor is a materialized resource of the consumer stream. It accepts sequenced elements, completion, failure, cancellation, and partner lifecycle signals, then updates the sink-side materialized stream state.

The actor-backed `SinkRef<T>` does not own the consumer collection state directly. It only points at the endpoint actor that owns the materialized receiver state.

## Wake / Drive Contract

Endpoint actor message handling must not only update endpoint state. Every accepted state transition that can unblock the stream must also wake or drive the materialized stream.

The required transitions are:

- handshake accepted,
- cumulative demand increased,
- sequenced element accepted,
- normal completion accepted after sequence validation,
- remote failure accepted,
- local cancellation requested,
- partner termination observed.

The wake signal should target the materialized stream resource, not the remote transport. In the current interpreter architecture this means StreamRef endpoint stages need materializer context. The first code step for this change is now in place: `SourceLogic` and `SinkLogic` receive `ActorSystem` context from `GraphInterpreter::new_with_materializer_context`, with a regression test proving the hook is called for both source and sink logic.

## Endpoint State Contract

`stream-core-kernel` now has a `StreamRefEndpointState` state holder for the actor-backed endpoint that will be created by the std/integration layer. This keeps protocol semantics in the no_std stream core while leaving actor spawning, DeathWatch, and remote wiring outside the core crate.

The state holder fixes three contracts that were previously implicit:

- the first valid partner is paired once and then fixed;
- a second partner pairing is reported as `StreamError::InvalidPartnerActor`, preserving the expected and received actor refs;
- protocol messages from a non-partner actor are also reported as `StreamError::InvalidPartnerActor`.

Completion, cancellation, and failure transition the state into a terminal condition and request endpoint shutdown. The local `StreamRefHandoff` now owns this state so the behavior is exercised by production code instead of remaining a test-only model.

## Endpoint Cleanup Contract

`StreamRefEndpointCleanup` is the cleanup hook for an actor-backed endpoint. It calls the real `StageActor::unwatch` and `StageActor::stop` paths instead of modeling cleanup as a boolean flag.

Cleanup failures are observable:

- partner watch release failure is recorded in `StreamRefEndpointState` and returned from cleanup;
- endpoint actor shutdown failure is recorded in `StreamRefEndpointState` and returned from cleanup;
- if both fail, the returned error preserves both failures with `StreamError::MaterializedResourceRollbackFailed`;
- terminal local handoff paths run the optional endpoint cleanup hook, so future actor-backed materialization can attach the real endpoint actor without changing terminal semantics.

## Actor-Backed Materialization Step

`StreamRefs::source_ref<T>()` now creates a shared endpoint slot for the materialized `SourceRef<T>`. During materializer context attachment, `StreamRefSinkLogic` creates a real `StageActor`, stores its `ActorRef` in that slot, and attaches the endpoint cleanup hook to the local handoff.

`StreamRefs::sink_ref<T>()` now mirrors the same endpoint ownership path for the materialized `SinkRef<T>`. During materializer context attachment, `StreamRefSourceLogic` creates a real `StageActor`, stores its `ActorRef` in the returned `SinkRef<T>`'s shared endpoint slot, and attaches the endpoint cleanup hook to the local handoff.

This proves the first actor-backed boundary without treating the local handoff as a serializable ref:

- a `SourceRef<T>` materialized by `ActorMaterializer` has a real endpoint actor identity;
- a `SinkRef<T>` materialized by `ActorMaterializer` has a real endpoint actor identity;
- their internal resolver helpers can derive the endpoint actor's canonical path;
- a missing endpoint actor still reports `StreamError::StreamRefTargetNotInitialized`;
- endpoint shutdown continues through the cleanup hook already covered by task 3.4.

The remaining endpoint actor implementation still has to define provider-dispatch resolve, the concrete drive handle, and protocol message handling that carries elements through the endpoint actor. That is now the boundary for tasks 4.3 through 6.x, not an unclosed ref identity issue.

## Provider Dispatch Resolver Step

`StreamRefResolver` now provides the serializer/resolver boundary for actor-backed refs:

- `source_ref_to_format` and `sink_ref_to_format` derive canonical actor path strings from materialized endpoint actors;
- `resolve_source_ref<T>` parses the serialized path and calls `ActorSystem::resolve_actor_ref`, returning a `SourceRef<T>` backed by the resolved endpoint actor;
- `resolve_sink_ref<T>` follows the same provider-dispatch path and returns a `SinkRef<T>`;
- the resolved refs hold actor-backed endpoint state, not a newly created `StreamRefHandoff<T>`.

The focused tests prove both direct actor-backed refs and materialized `StreamRefs::source_ref` / `StreamRefs::sink_ref` refs round-trip through the local provider. They also send probe messages through the resolved endpoint `ActorRef` and drain the original `StageActor`, proving loopback resolution stays on the normal local actor delivery path.

The resolver now fails explicitly for unsupported local-only refs, missing endpoint actors, and invalid path formats. Protocol delivery from an actor-backed `into_source` / `into_sink` remains intentionally unimplemented; those paths fail explicitly until the protocol message handling tasks connect stream demand, element, and terminal signals.
