//! `StdRemoteActorRefProvider` — adapter that dispatches an `ActorPath` to
//! either actor-core's local provider or `remote-core`'s remote provider per
//! design Decision 3-C.

use alloc::boxed::Box;
use std::time::Instant;

use fraktor_actor_core_rs::core::kernel::{
  actor::{
    Pid,
    actor_path::{ActorPath, ActorPathScheme},
    actor_ref::ActorRef,
    actor_ref_provider::{ActorRefProvider, ActorRefProviderHandleShared, LocalActorRefProvider},
    error::ActorError,
  },
  serialization::{ActorRefResolveCache, ActorRefResolveCacheOutcome as ActorCoreResolveCacheOutcome},
  system::TerminationSignal,
};
use fraktor_remote_core_rs::core::{
  address::UniqueAddress,
  extension::{
    EventPublisher, REMOTE_ACTOR_REF_RESOLVE_CACHE_EXTENSION, RemoteActorRefResolveCacheEvent,
    RemoteActorRefResolveCacheOutcome, RemoteEvent,
  },
  provider::{RemoteActorRef, RemoteActorRefProvider, resolve_remote_address},
};
use tokio::sync::mpsc::Sender;

use crate::std::provider::{
  provider_dispatch_error::StdRemoteActorRefProviderError, remote_actor_ref_sender::RemoteActorRefSender,
};

// remote actor ref は PID 空間の上位 1/4 を利用し、runtime allocator が
// 0 から払い出す local actor PID と分離する。この値を変更する場合は
// local 側 allocator との調整が必要。
const REMOTE_ACTOR_REF_PID_START: u64 = u64::MAX / 4;
// std アダプタが現在 materialize する remote path scheme は fraktor.tcp のみ。
const SUPPORTED_SCHEMES: [ActorPathScheme; 1] = [ActorPathScheme::FraktorTcp];

/// `std + tokio` actor ref provider that performs the loopback / remote
/// dispatch demanded by design Decision 3-C.
///
/// `StdRemoteActorRefProvider::actor_ref` follows the three-branch dispatch
/// rule from the spec:
///
/// 1. **No authority** (`authority_endpoint().is_none()`) → forward to the local provider
///    unchanged.
/// 2. **Authority matches the local one** (host/port/system equal, with `uid == 0` treated as a
///    wildcard per design Decision 13) → strip the authority and forward the local-equivalent path
///    to the local provider.
/// 3. **Authority does not match** → resolve through the core remote provider, build a
///    [`crate::std::provider::RemoteActorRefSender`], and wrap it as an `ActorRef`.
///
/// `watch` and `unwatch` are remote-only — local death-watch is handled by
/// actor-core's normal `ActorContext::watch` path on the resolved local
/// `ActorRef`.
pub struct StdRemoteActorRefProvider {
  local_address:   UniqueAddress,
  local_provider:  ActorRefProviderHandleShared<LocalActorRefProvider>,
  remote_provider: Box<dyn RemoteActorRefProvider + Send + Sync>,
  event_sender:    Sender<RemoteEvent>,
  resolve_cache:   ActorRefResolveCache<ActorRef>,
  event_publisher: EventPublisher,
  monotonic_epoch: Instant,
  next_remote_pid: u64,
}

impl StdRemoteActorRefProvider {
  /// Creates a new dispatcher.
  #[must_use]
  pub fn new(
    local_address: UniqueAddress,
    local_provider: ActorRefProviderHandleShared<LocalActorRefProvider>,
    remote_provider: Box<dyn RemoteActorRefProvider + Send + Sync>,
    event_sender: Sender<RemoteEvent>,
    resolve_cache: ActorRefResolveCache<ActorRef>,
    event_publisher: EventPublisher,
    monotonic_epoch: Instant,
  ) -> Self {
    Self {
      local_address,
      local_provider,
      remote_provider,
      event_sender,
      resolve_cache,
      event_publisher,
      monotonic_epoch,
      next_remote_pid: REMOTE_ACTOR_REF_PID_START,
    }
  }

  /// Returns the local [`UniqueAddress`] used to determine the loopback
  /// branch.
  #[must_use]
  pub const fn local_address(&self) -> &UniqueAddress {
    &self.local_address
  }

  /// Resolves an [`ActorPath`] into an actor-core [`ActorRef`].
  ///
  /// # Errors
  ///
  /// Returns [`StdRemoteActorRefProviderError`] when:
  ///
  /// - the local provider rejects the path (`LocalProvider`),
  /// - the core remote provider rejects the path (`CoreProvider`), or
  /// - the adapter exhausts its synthetic pid space for remote references.
  pub fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, StdRemoteActorRefProviderError> {
    if path.parts().authority_endpoint().is_none() {
      // Branch 1: authority がなければ local provider へそのまま委譲する。
      return self.local_provider.actor_ref(path).map_err(StdRemoteActorRefProviderError::from);
    }
    if let Some(resolved) = resolve_remote_address(&path)
      && self.is_local_authority(&resolved)
    {
      // Branch 2: authority が local node と一致する場合は authority を落とし、
      // local 等価な path として local provider へ委譲する。
      let local_path = strip_authority(path);
      return self.local_provider.actor_ref(local_path).map_err(StdRemoteActorRefProviderError::from);
    }
    // Branch 3: authority が一致しない場合は core remote provider へ委譲する。
    let outcome = self.resolve_remote_actor_ref(path.clone())?;
    self.publish_resolve_cache_event(path, remote_cache_outcome(&outcome));
    Ok(match outcome {
      | ActorCoreResolveCacheOutcome::Hit(actor_ref) | ActorCoreResolveCacheOutcome::Miss(actor_ref) => actor_ref,
    })
  }

  /// Registers a remote death-watch.
  ///
  /// # Errors
  ///
  /// Returns [`StdRemoteActorRefProviderError::NotRemote`] if `watchee` is a
  /// local actor path. Local death-watch must be performed via the normal
  /// actor-core `ActorContext::watch` path on the resolved local
  /// [`ActorRef`].
  pub fn watch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), StdRemoteActorRefProviderError> {
    if !self.is_remote_path(&watchee) {
      return Err(StdRemoteActorRefProviderError::NotRemote);
    }
    self.remote_provider.watch(watchee, watcher).map_err(StdRemoteActorRefProviderError::from)
  }

  /// Cancels a previously registered remote death-watch.
  ///
  /// # Errors
  ///
  /// Mirrors [`Self::watch`] — local watchees return
  /// [`StdRemoteActorRefProviderError::NotRemote`].
  pub fn unwatch(&mut self, watchee: ActorPath, watcher: Pid) -> Result<(), StdRemoteActorRefProviderError> {
    if !self.is_remote_path(&watchee) {
      return Err(StdRemoteActorRefProviderError::NotRemote);
    }
    self.remote_provider.unwatch(watchee, watcher).map_err(StdRemoteActorRefProviderError::from)
  }

  fn is_local_authority(&self, resolved: &UniqueAddress) -> bool {
    if resolved.address() != self.local_address.address() {
      return false;
    }
    // design Decision 13 により uid == 0 は wildcard として扱う。
    resolved.uid() == 0 || resolved.uid() == self.local_address.uid()
  }

  fn is_remote_path(&self, path: &ActorPath) -> bool {
    let Some(resolved) = resolve_remote_address(path) else {
      return false;
    };
    !self.is_local_authority(&resolved)
  }

  fn resolve_remote_actor_ref(
    &mut self,
    path: ActorPath,
  ) -> Result<ActorCoreResolveCacheOutcome<ActorRef>, StdRemoteActorRefProviderError> {
    let remote_provider = &mut self.remote_provider;
    let next_remote_pid = &mut self.next_remote_pid;
    let event_sender = self.event_sender.clone();
    let monotonic_epoch = self.monotonic_epoch;
    self.resolve_cache.resolve(&path, |candidate| {
      let remote_ref = remote_provider.actor_ref(candidate.clone()).map_err(StdRemoteActorRefProviderError::from)?;
      Self::build_remote_actor_ref(next_remote_pid, remote_ref, event_sender.clone(), monotonic_epoch)
    })
  }

  fn build_remote_actor_ref(
    next_remote_pid: &mut u64,
    remote_ref: RemoteActorRef,
    event_sender: Sender<RemoteEvent>,
    monotonic_epoch: Instant,
  ) -> Result<ActorRef, StdRemoteActorRefProviderError> {
    let pid = Self::allocate_remote_pid(next_remote_pid)?;
    let path = remote_ref.path().clone();
    let sender = RemoteActorRefSender::new(remote_ref, event_sender, monotonic_epoch);
    Ok(ActorRef::with_canonical_path(pid, sender, path))
  }

  fn allocate_remote_pid(next_remote_pid: &mut u64) -> Result<Pid, StdRemoteActorRefProviderError> {
    let pid = Pid::new(*next_remote_pid, 0);
    *next_remote_pid = next_remote_pid.checked_add(1).ok_or(StdRemoteActorRefProviderError::RemotePidExhausted)?;
    Ok(pid)
  }

  fn publish_resolve_cache_event(&self, path: ActorPath, outcome: RemoteActorRefResolveCacheOutcome) {
    self
      .event_publisher
      .publish_extension(REMOTE_ACTOR_REF_RESOLVE_CACHE_EXTENSION, RemoteActorRefResolveCacheEvent::new(path, outcome));
  }
}

impl ActorRefProvider for StdRemoteActorRefProvider {
  fn supported_schemes(&self) -> &'static [ActorPathScheme] {
    &SUPPORTED_SCHEMES
  }

  fn actor_ref(&mut self, path: ActorPath) -> Result<ActorRef, ActorError> {
    StdRemoteActorRefProvider::actor_ref(self, path).map_err(StdRemoteActorRefProviderError::into_actor_error)
  }

  fn termination_signal(&self) -> TerminationSignal {
    self.local_provider.termination_signal()
  }
}

fn remote_cache_outcome<T>(outcome: &ActorCoreResolveCacheOutcome<T>) -> RemoteActorRefResolveCacheOutcome {
  match outcome {
    | ActorCoreResolveCacheOutcome::Hit(_) => RemoteActorRefResolveCacheOutcome::Hit,
    | ActorCoreResolveCacheOutcome::Miss(_) => RemoteActorRefResolveCacheOutcome::Miss,
  }
}

/// Returns a copy of `path` with its authority component stripped.
///
/// Authority-less paths are unchanged.
fn strip_authority(path: ActorPath) -> ActorPath {
  let segments = path.segments().iter().map(|segment| segment.as_str());
  ActorPath::try_from_segments(segments).expect("try_from_segments must succeed for a path with valid segments")
}
