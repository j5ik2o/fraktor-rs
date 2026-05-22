## ADDED Requirements

### Requirement: StreamRef resolver uses remote-aware actor-ref provider dispatch

The std StreamRef resolver SHALL use the installed remote-aware actor-ref provider to resolve serialized StreamRef endpoint actor paths. It MUST NOT bypass provider dispatch by constructing `RemoteActorRefSender`, `RemoteEvent`, `TcpRemoteTransport`, or transport endpoints directly.

#### Scenario: resolver uses ActorSystem provider surface

- **GIVEN** a remote-aware actor-ref provider is installed in the ActorSystem
- **WHEN** the StreamRef resolver resolves a serialized SourceRef or SinkRef actor path
- **THEN** it calls the ActorSystem actor-ref resolution surface or an equivalent provider-dispatch API
- **AND** the returned ActorRef is the endpoint used by the remote StreamRef wrapper

#### Scenario: resolver does not construct transport directly

- **WHEN** the StreamRef resolver implementation is inspected
- **THEN** it does not instantiate `TcpRemoteTransport`
- **AND** it does not enqueue `RemoteEvent::OutboundEnqueued` directly
- **AND** it does not parse host and port into a transport connection outside provider dispatch

#### Scenario: loopback serialized path stays local

- **GIVEN** a serialized StreamRef endpoint path whose authority resolves to the local ActorSystem
- **WHEN** the StreamRef resolver resolves that path
- **THEN** provider dispatch applies the normal loopback rule
- **AND** the resulting endpoint uses local actor delivery rather than remote TCP delivery

#### Scenario: remote serialized path uses remote sender

- **GIVEN** a serialized StreamRef endpoint path whose authority does not match the local ActorSystem
- **WHEN** the StreamRef resolver resolves that path
- **THEN** provider dispatch materializes a remote ActorRef
- **AND** sending StreamRef protocol payloads to that ActorRef reaches the existing remote outbound envelope path

### Requirement: remote watch integration covers StreamRef endpoint partners

Remote StreamRef endpoint actors SHALL participate in the existing remote watch integration so partner termination can fail the materialized stream.

#### Scenario: endpoint watches partner actor

- **WHEN** a remote StreamRef endpoint accepts a partner handshake
- **THEN** it registers a watch for the partner actor through the actor system watch path
- **AND** remote watch hook dispatch handles remote partners using existing provider mapping

#### Scenario: unpairing removes partner watch

- **WHEN** a remote StreamRef endpoint reaches normal completion, cancellation, or failure
- **THEN** it unregisters or otherwise releases the partner watch through the existing actor watch path
- **AND** any failure to release the watch is observable rather than silently ignored

#### Scenario: DeathWatch notification is routed to endpoint

- **WHEN** the remote partner terminates before protocol completion
- **THEN** the existing remote watch hook delivers a DeathWatch notification to the StreamRef endpoint actor
- **AND** the endpoint fails the stream instead of completing it normally
