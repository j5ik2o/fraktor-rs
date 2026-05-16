## 1. Baseline and Contract Review

- [ ] 1.1 Re-read Pekko `StreamRefs`, `StreamRefResolverImpl`, `StreamRefsMaster`, `SourceRefImpl`, `SinkRefImpl`, and stream ref protocol references; record only implementation-relevant notes in tests or code comments.
- [ ] 1.2 Inspect current `stream-core-kernel` StreamRef local handoff, protocol, settings, and error mapping before editing.
- [ ] 1.3 Inspect current remote provider dispatch, remote watch hook, serialization registry, and TCP outbound envelope path before editing.
- [ ] 1.4 Run focused baseline tests for StreamRef, stream backpressure, remote provider dispatch, remote watch hook, and serialization paths.

## 2. StreamRef Core Boundary

- [ ] 2.1 Keep `stream-core-kernel` free of `remote-core`, `remote-adaptor-std`, tokio, and std-only dependencies while adding any protocol or error surface needed by remote StreamRef.
- [ ] 2.2 Add or adjust StreamRef protocol tests for sequence validation, invalid demand, remote failure, and terminal ordering that are shared by local and remote handoff.
- [ ] 2.3 Preserve existing local `SourceRef::into_source` and `SinkRef::into_sink` semantics while making remote-capable endpoint ownership explicit.
- [ ] 2.4 Add tests proving local handoff still works without remote runtime installation.

## 3. Resolver and Serialization Integration

- [ ] 3.1 Implement StreamRef resolver installation in the std adaptor or integration layer without adding remote dependencies to stream-core.
- [ ] 3.2 Convert remote-capable `SourceRef` and `SinkRef` to canonical endpoint actor path strings using actor-core path APIs.
- [ ] 3.3 Resolve serialized SourceRef and SinkRef strings through ActorSystem actor-ref provider dispatch, including loopback and remote authority cases.
- [ ] 3.4 Register StreamRef protocol payload serializers or equivalent manifest routes required for remote delivery.
- [ ] 3.5 Add tests for missing serializer registration and unsupported ref implementation failures.

## 4. Remote Endpoint Actor Wiring

- [ ] 4.1 Materialize SourceRef and SinkRef endpoint actors as owned stream resources with deterministic shutdown on completion, cancellation, and failure.
- [ ] 4.2 Route cumulative demand, sequenced elements, handshake, completion, failure, and cancellation through normal remote ActorRef delivery.
- [ ] 4.3 Enforce one-shot partner pairing and reject double materialization or messages from a non-partner actor.
- [ ] 4.4 Connect endpoint partner watches through the existing actor watch path and remote watch hook.
- [ ] 4.5 Ensure watch release failures, send failures, and endpoint shutdown failures are observable and not silently ignored.

## 5. Backpressure and Failure Semantics

- [ ] 5.1 Verify elements are not delivered without remote cumulative demand.
- [ ] 5.2 Preserve pending elements when transport enqueue reports backpressure, or fail the stream with an observable error.
- [ ] 5.3 Verify completion is observed only after pending sequenced elements are delivered.
- [ ] 5.4 Map partner DeathWatch notification, address termination, transport connection loss, invalid sequence, invalid demand, and invalid partner to distinct observable stream failures.
- [ ] 5.5 Verify cancellation propagates to the remote partner and prevents further element publication for that ref.

## 6. Integration Tests and Documentation

- [ ] 6.1 Add two-ActorSystem integration tests for passing a SourceRef as a remote message payload and streaming elements with backpressure.
- [ ] 6.2 Add two-ActorSystem integration tests for passing a SinkRef as a remote message payload and streaming elements with backpressure.
- [ ] 6.3 Add remote failure integration tests for partner termination and address termination before protocol completion.
- [ ] 6.4 Update `docs/gap-analysis/stream-gap-analysis.md` to reflect the StreamRef remote gap status after implementation.
- [ ] 6.5 Run targeted crate tests, `mise exec -- openspec validate add-remote-stream-ref-transport --strict`, and `git diff --check`.
- [ ] 6.6 Unless a narrower verification scope is explicitly approved, run `./scripts/ci-check.sh ai all` before marking the change complete.
