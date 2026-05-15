## 1. Baseline and Contract Confirmation

- [ ] 1.1 Confirm Pekko `RemoteWatcher` / `RemoteDaemon` address termination behavior and record the relevant reference lines in implementation notes or test comments.
- [ ] 1.2 Confirm current fraktor event stream variants, classifier keys, watcher effects, and std watcher task boundaries before editing.
- [ ] 1.3 Run targeted baseline tests for actor-core event stream, remote-core watcher state, and remote-adaptor-std watcher / deployment paths.

## 2. actor-core Event Stream Surface

- [ ] 2.1 Add an `AddressTerminated` event payload type that carries the terminated remote authority string, reason, and monotonic millis observation timestamp without using std-only types or depending on `remote-core`.
- [ ] 2.2 Add `EventStreamEvent::AddressTerminated`, `ClassifierKey::AddressTerminated`, public re-exports, and clone / classifier mapping support.
- [ ] 2.3 Update event stream classifier and subchannel tests so `AddressTerminated` is covered by concrete-key subscriptions and `ClassifierKey::All`.
- [ ] 2.4 Keep actor-core no_std compatibility and avoid introducing std runtime dependencies into event stream types.

## 3. remote-core Watcher State

- [ ] 3.1 Add a watcher effect for address-level termination that includes the unavailable `remote-core` address, reason metadata, and monotonic millis observation timestamp for std publication.
- [ ] 3.2 Emit the address termination effect from `WatcherState::handle(HeartbeatTick)` when a remote node first becomes unavailable in a failure epoch.
- [ ] 3.3 Preserve existing `NotifyTerminated` effects for watched remote actors when address termination is emitted.
- [ ] 3.4 Add watcher state tests for one-shot address termination emission, repeated tick suppression, and heartbeat / heartbeat-response reset.

## 4. std Watcher Publication

- [ ] 4.1 Apply the new watcher effect in `remote-adaptor-std` by mapping the `remote-core` address to an actor-core authority string and publishing `EventStreamEvent::AddressTerminated` through the actor system event stream.
- [ ] 4.2 Keep local watcher `DeathWatchNotification` delivery on the existing actor-core path and verify it is not replaced by address termination publication.
- [ ] 4.3 Add std watcher tests for address termination publication, event classifier filtering, and simultaneous DeathWatch notification delivery.

## 5. Remote Deployment Cleanup

- [ ] 5.1 Subscribe remote deployment watcher / dispatcher state to `ClassifierKey::AddressTerminated` rather than calling deployment code directly from the watcher task.
- [ ] 5.2 Track pending deployment start timestamps and ignore replayed address termination events older than the pending request.
- [ ] 5.3 Fail pending deployment requests for the terminated authority with an address-termination-specific error instead of a timeout.
- [ ] 5.4 Reject late deployment responses for cleaned-up correlation ids as stale responses and keep the deployment in the failed state.
- [ ] 5.5 Add unit or integration tests for pending deployment failure, replayed old termination suppression, and late response rejection after address termination.

## 6. Integration, Specs, and Documentation

- [ ] 6.1 Add a targeted integration test showing remote node failure publishes address termination and still notifies local DeathWatch watchers.
- [ ] 6.2 Update `docs/gap-analysis/remote-gap-analysis.md` after implementation to remove the `AddressTerminated` residual gap.
- [ ] 6.3 Run targeted tests for affected crates, then `mise exec -- openspec validate add-address-terminated-integration --strict` and `git diff --check`.
- [ ] 6.4 Run `./scripts/ci-check.sh ai all` before marking the change complete, unless a narrower user-approved verification scope is explicitly chosen.
