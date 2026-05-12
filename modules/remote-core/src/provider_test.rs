use alloc::vec::Vec;

use fraktor_actor_core_kernel_rs::actor::{
  Pid,
  actor_path::{ActorPath, ActorPathParser},
};

use crate::{
  address::{Address, RemoteNodeId, UniqueAddress},
  provider::{ProviderError, RemoteActorRef, RemoteActorRefProvider, resolve_remote_address},
};

// ---------------------------------------------------------------------------
// path_resolver::resolve_remote_address
// ---------------------------------------------------------------------------

#[test]
fn resolve_returns_none_for_local_path() {
  let p = ActorPath::root().child("user").child("local");
  assert!(resolve_remote_address(&p).is_none());
}

#[test]
fn resolve_returns_address_for_remote_path() {
  let p = ActorPathParser::parse("fraktor.tcp://sys@10.0.0.1:2552/user/worker").expect("parse");
  let ua = resolve_remote_address(&p).expect("remote address");
  assert_eq!(ua.address(), &Address::new("sys", "10.0.0.1", 2552));
  // The uid may be `0` when the path carries none — `uid == 0` is the
  // spec-defined "unconfirmed" sentinel per Decision 13.
  assert_eq!(ua.uid(), 0);
}

#[test]
fn resolve_honours_path_uid_when_present() {
  let p = ActorPathParser::parse("fraktor.tcp://sys@10.0.0.1:2552/user/worker#42").expect("parse");
  let ua = resolve_remote_address(&p).expect("resolved");
  assert_eq!(ua.uid(), 42);
}

// ---------------------------------------------------------------------------
// RemoteActorRef data type
// ---------------------------------------------------------------------------

#[test]
fn remote_actor_ref_accessors() {
  let path = ActorPathParser::parse("fraktor.tcp://sys@host:2552/user/x").expect("parse");
  let node = RemoteNodeId::new("sys", "host", Some(2552), 7);
  let r = RemoteActorRef::new(path.clone(), node.clone());
  assert_eq!(r.path(), &path);
  assert_eq!(r.remote_node(), &node);
}

#[test]
fn remote_actor_ref_clone_and_equality() {
  let path = ActorPathParser::parse("fraktor.tcp://sys@host:2552/user/x").expect("parse");
  let node = RemoteNodeId::new("sys", "host", Some(2552), 7);
  let a = RemoteActorRef::new(path.clone(), node.clone());
  let b = a.clone();
  assert_eq!(a, b);
  // A different path produces an inequal ref.
  let other_path = ActorPathParser::parse("fraktor.tcp://sys@host:2552/user/y").expect("parse");
  let c = RemoteActorRef::new(other_path, node);
  assert_ne!(a, c);
}

// ---------------------------------------------------------------------------
// RemoteActorRefProvider contract — exercised via a tiny stub implementation
// ---------------------------------------------------------------------------

struct StubProvider {
  local_authority: UniqueAddress,
  watch_calls:     Vec<(ActorPath, Pid)>,
  unwatch_calls:   Vec<(ActorPath, Pid)>,
}

impl StubProvider {
  fn new(local_authority: UniqueAddress) -> Self {
    Self { local_authority, watch_calls: Vec::new(), unwatch_calls: Vec::new() }
  }
}

impl RemoteActorRefProvider for StubProvider {
  fn actor_ref(&mut self, path: ActorPath) -> Result<RemoteActorRef, ProviderError> {
    let Some(resolved) = resolve_remote_address(&path) else {
      return Err(ProviderError::MissingAuthority);
    };
    if resolved.address() == self.local_authority.address() {
      // Adapter should have filtered this out before calling us.
      return Err(ProviderError::NotRemote);
    }
    let node = RemoteNodeId::new(
      resolved.address().system(),
      resolved.address().host(),
      Some(resolved.address().port()),
      resolved.uid(),
    );
    Ok(RemoteActorRef::new(path, node))
  }

  fn watch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError> {
    if resolve_remote_address(&watchee).map(|r| r.address() == self.local_authority.address()).unwrap_or(true) {
      return Err(ProviderError::NotRemote);
    }
    self.watch_calls.push((watchee, watcher));
    Ok(())
  }

  fn unwatch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), ProviderError> {
    if resolve_remote_address(&watchee).map(|r| r.address() == self.local_authority.address()).unwrap_or(true) {
      return Err(ProviderError::NotRemote);
    }
    self.unwatch_calls.push((watchee, watcher));
    Ok(())
  }
}

fn stub_provider() -> StubProvider {
  StubProvider::new(UniqueAddress::new(Address::new("local-sys", "127.0.0.1", 2551), 1))
}

#[test]
fn actor_ref_returns_remote_ref_for_remote_path() {
  let mut provider = stub_provider();
  let path = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let r = provider.actor_ref(path.clone()).expect("resolve");
  assert_eq!(r.path(), &path);
  assert_eq!(r.remote_node().system(), "remote-sys");
}

#[test]
fn actor_ref_rejects_local_path_with_not_remote() {
  let mut provider = stub_provider();
  let local = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker").expect("parse");
  let err = provider.actor_ref(local).unwrap_err();
  assert_eq!(err, ProviderError::NotRemote);
}

#[test]
fn actor_ref_rejects_authorityless_path() {
  let mut provider = stub_provider();
  let local = ActorPath::root().child("user").child("worker");
  let err = provider.actor_ref(local).unwrap_err();
  assert_eq!(err, ProviderError::MissingAuthority);
}

#[test]
fn watch_and_unwatch_record_remote_targets() {
  let mut provider = stub_provider();
  let watchee = ActorPathParser::parse("fraktor.tcp://remote-sys@10.0.0.1:2552/user/worker").expect("parse");
  let watcher = Pid::new(1, 1);
  provider.watch(watchee.clone(), watcher).unwrap();
  assert_eq!(provider.watch_calls.len(), 1);
  provider.unwatch(watchee, watcher).unwrap();
  assert_eq!(provider.unwatch_calls.len(), 1);
}

#[test]
fn watch_rejects_local_watchee() {
  let mut provider = stub_provider();
  let local = ActorPathParser::parse("fraktor.tcp://local-sys@127.0.0.1:2551/user/worker").expect("parse");
  let watcher = Pid::new(1, 1);
  assert_eq!(provider.watch(local.clone(), watcher).unwrap_err(), ProviderError::NotRemote);
  assert_eq!(provider.unwatch(local, watcher).unwrap_err(), ProviderError::NotRemote);
}
