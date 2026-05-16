## ADDED Requirements

### Requirement: remote StreamRef preserves backpressure and terminal ordering

Remote StreamRef handoff SHALL preserve stream-level backpressure, pending elements, completion, failure, and cancellation across the actor remoting boundary. Transport enqueue backpressure MUST be observable separately from stream-level demand and MUST NOT silently drop accepted stream elements.

#### Scenario: elements are not sent without demand

- **WHEN** a remote SourceRef endpoint has buffered elements but has not received cumulative demand from the partner
- **THEN** it does not send those elements as accepted downstream delivery
- **AND** the elements remain pending until demand or terminal failure changes the state

#### Scenario: accepted element is not lost on transport backpressure

- **WHEN** a remote StreamRef endpoint has an accepted element and remote actor delivery reports transport backpressure
- **THEN** the endpoint keeps the element pending or fails the stream with an observable transport error
- **AND** it does not silently discard the element

#### Scenario: completion is delivered after pending elements

- **WHEN** the producing side completes after sending one or more sequenced elements
- **THEN** the remote partner observes all valid pending elements before normal completion
- **AND** completion is not reordered ahead of elements that passed sequence validation

#### Scenario: failure takes precedence over normal completion

- **WHEN** a remote StreamRef endpoint observes remote failure, invalid sequence, invalid partner, or partner termination before protocol completion is accepted
- **THEN** the materialized stream fails
- **AND** it does not report normal completion for the same connection

#### Scenario: cancellation propagates to the remote partner

- **WHEN** the local materialized stream is cancelled before normal completion
- **THEN** the remote StreamRef endpoint sends a cancellation or terminal failure signal to its partner
- **AND** the partner stops publishing additional elements for that ref
