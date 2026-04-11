use alloc::vec::Vec;
use std::sync::{Arc, Mutex};

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Pid,
    actor_path::{ActorPath, ActorPathParser},
    actor_ref_provider::{ActorRefProviderHandleSharedFactory, LocalActorRefProvider},
  },
  system::shared_factory::BuiltinSpinSharedFactory,
};
use fraktor_remote_core_rs::{
  address::{Address, RemoteNodeId, UniqueAddress},
  provider::{ProviderError, RemoteActorRef, RemoteActorRefProvider},
};

use crate::{
  provider::{dispatch::StdRemoteActorRefProvider, provider_dispatch_error::StdRemoteActorRefProviderError},
  tcp_transport::TcpRemoteTransport,
};

// ---------------------------------------------------------------------------
// Test fixtures
// ---------------------------------------------------------------------------

/// Tracks every call so tests can assert the dispatch path.
#[derive(Default)]
struct StubRemoteProvider {
  actor_ref_calls: Vec<ActorPath>,
  watch_calls:     Vec<(ActorPath, Pid)>,
  unwatch_calls:   Vec<(ActorPath, Pid)>,
}

impl RemoteActorRefProvider for StubRemoteProvider {
  fn actor_ref(&mut self, path: ActorPath) -> Result<RemoteActorRef, ProviderError> {
    self.actor_ref_calls.push(path.clone());
    let node = RemoteNodeId::new("remote", "10.0.0.1", Some(2552), 1);
    Ok(RemoteActorRef::new(path, node))
  }

  fn watch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError> {
    self.watch_calls.push((watchee, watcher));
    Ok(())
  }

  fn unwatch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError> {
    self.unwatch_calls.push((watchee, watcher));
    Ok(())
  }
}

fn local_address() -> UniqueAddress {
  UniqueAddress::new(Address::new("local-sys", "127.0.0.1", 2551), 7)
}

fn make_provider() -> StdRemoteActorRefProvider {
  let local_provider =
    ActorRefProviderHandleSharedFactory::create(&BuiltinSpinSharedFactory::new(), LocalActorRefProvider::new());
  let remote_provider = Box::new(StubRemoteProvider::default()) as Box<dyn RemoteActorRefProvider + Send + Sync>;
  let transport = Arc::new(Mutex::new(TcpRemoteTransport::new("127.0.0.1:0", Vec::new())));
  StdRemoteActorRefProvider::new(local_address(), local_provider, remote_provider, transport)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn local_path_without_authority_is_dispatched_to_local_provider() {
  let mut provider = make_provider();
  let local_path = ActorPath::root().child("user").child("worker");
  // The unconfigured local provider returns an error, but we only care that
  // the call lands on `LocalProvider(...)` (not on `CoreProvider`).
  let err = provider.actor_ref(local_path).unwrap_err();
  assert!(matches!(err, StdRemoteActorRefProviderError::LocalProvider(_)), "expected LocalProvider error, got {err:?}");
}

#[test]
fn remote_path_with_non_matching_authority_is_dispatched_to_remote_provider() {
  let mut provider = make_provider();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.99:2552/user/worker").expect("parse");
  let err = provider.actor_ref(remote_path).unwrap_err();
  // Phase B minimum-viable returns `RemoteSenderBuildFailed` after a
  // successful core resolve — that proves the dispatch went through the
  // remote branch.
  assert!(
    matches!(err, StdRemoteActorRefProviderError::RemoteSenderBuildFailed),
    "expected RemoteSenderBuildFailed (remote branch), got {err:?}"
  );
}

#[test]
fn local_authority_path_is_normalized_to_local_provider() {
  let mut provider = make_provider();
  // Authority that exactly matches `local_address()`.
  let local_path = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker").expect("parse");
  let err = provider.actor_ref(local_path).unwrap_err();
  assert!(
    matches!(err, StdRemoteActorRefProviderError::LocalProvider(_)),
    "expected LocalProvider error (loopback dispatched to local provider), got {err:?}"
  );
}

#[test]
fn local_authority_path_with_uid_zero_is_treated_as_wildcard() {
  let mut provider = make_provider();
  // `#0` UID is a wildcard per design Decision 13 — Address match alone
  // should still trigger the loopback branch even though the local UID is 7.
  let local_path = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker#0").expect("parse");
  let err = provider.actor_ref(local_path).unwrap_err();
  assert!(
    matches!(err, StdRemoteActorRefProviderError::LocalProvider(_)),
    "expected LocalProvider error (wildcard UID dispatched to local), got {err:?}"
  );
}

#[test]
fn local_authority_path_with_non_matching_uid_is_dispatched_to_remote() {
  let mut provider = make_provider();
  // Same Address as local but UID = 99 (non-zero, non-matching) — should
  // route to the remote branch.
  let local_path = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker#99").expect("parse");
  let err = provider.actor_ref(local_path).unwrap_err();
  assert!(
    matches!(err, StdRemoteActorRefProviderError::RemoteSenderBuildFailed),
    "expected RemoteSenderBuildFailed (remote branch via UID mismatch), got {err:?}"
  );
}

#[test]
fn watch_remote_path_forwards_to_remote_provider() {
  let mut provider = make_provider();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  provider.watch(remote_path, Pid::new(1, 1)).expect("watch should succeed");
}

#[test]
fn unwatch_remote_path_forwards_to_remote_provider() {
  let mut provider = make_provider();
  let remote_path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  provider.unwatch(remote_path, Pid::new(1, 1)).expect("unwatch should succeed");
}

#[test]
fn watch_local_path_returns_not_remote() {
  let mut provider = make_provider();
  let local_path = ActorPath::root().child("user").child("worker");
  let err = provider.watch(local_path, Pid::new(1, 1)).unwrap_err();
  assert!(matches!(err, StdRemoteActorRefProviderError::NotRemote));
}

#[test]
fn unwatch_local_path_returns_not_remote() {
  let mut provider = make_provider();
  let local_path = ActorPath::root().child("user").child("worker");
  let err = provider.unwatch(local_path, Pid::new(1, 1)).unwrap_err();
  assert!(matches!(err, StdRemoteActorRefProviderError::NotRemote));
}

#[test]
fn watch_local_authority_path_returns_not_remote() {
  let mut provider = make_provider();
  // Authority matches local — should be treated as local for watch purposes.
  let local_path = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker").expect("parse");
  let err = provider.watch(local_path, Pid::new(1, 1)).unwrap_err();
  assert!(matches!(err, StdRemoteActorRefProviderError::NotRemote));
}
