# remote-graceful-flush implementation plan

## Objective

Implement OpenSpec change `remote-graceful-flush` with minimal, task-tracked changes.

## Plan

1. Add wire-level flush control PDUs and codec coverage.
   Verify with `fraktor-remote-core-rs` wire tests.
2. Add core association flush session state and transport-neutral outcomes.
   Verify duplicate ack, timeout, connection loss, and pending queue behavior with core unit tests.
3. Add `Remote` / `RemoteShared` flush start, timer input, inbound ack/request handling, and outcome observation.
   Verify the std adaptor can consume outcomes without exposing raw locks or association references.
4. Add lane-targeted flush delivery to `RemoteTransport` and `TcpRemoteTransport`.
   Verify writer-lane targeting, lane 0 handling, backpressure, inbound request routing, and ack send failures.
5. Add std flush gate for remote-bound `DeathWatchNotification` and update `RemotingExtensionInstaller::shutdown_and_join`.
   Verify timeout/start-failure/completion all release pending shutdown or notification paths.
6. Run the OpenSpec-requested verification commands and update the remote gap analysis when implementation is complete.
