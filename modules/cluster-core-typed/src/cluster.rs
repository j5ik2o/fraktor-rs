//! Typed cluster access point.

#[cfg(test)]
#[path = "cluster_test.rs"]
mod tests;

use alloc::string::String;
use core::any::Any;

use fraktor_actor_core_kernel_rs::event::stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle};
use fraktor_actor_core_typed_rs::{TypedActorRef, TypedActorSystem};
use fraktor_cluster_core_kernel_rs::{
  extension::{ClusterApi, ClusterApiError, ClusterError, ClusterSubscriptionInitialStateMode},
  grain::GrainRef as KernelGrainRef,
  membership::{CurrentClusterState, NodeStatus},
  topology::{ClusterEvent, ClusterEventType},
};
use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

use crate::{ClusterEventSubscription, ClusterIdentity, GrainRef, SelfRemoved, SelfUp};

struct TypedClusterEventSubscriber {
  subscriber:        TypedActorRef<ClusterEvent>,
  failed_deliveries: SharedLock<u64>,
}

impl TypedClusterEventSubscriber {
  const fn new(subscriber: TypedActorRef<ClusterEvent>, failed_deliveries: SharedLock<u64>) -> Self {
    Self { subscriber, failed_deliveries }
  }

  fn deliver(&mut self, cluster_event: &ClusterEvent) {
    if self.subscriber.try_tell(cluster_event.clone()).is_err() {
      record_failed_delivery(&self.failed_deliveries);
    }
  }
}

impl EventStreamSubscriber for TypedClusterEventSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { payload, .. } = event
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
    {
      self.deliver(cluster_event);
    }
  }
}

struct SelfUpSubscriber {
  subscriber:        TypedActorRef<SelfUp>,
  self_authority:    String,
  cluster:           ClusterApi,
  state:             SharedLock<SelfEventSubscriberState>,
  failed_deliveries: SharedLock<u64>,
}

impl SelfUpSubscriber {
  fn new(
    subscriber: TypedActorRef<SelfUp>,
    self_authority: String,
    cluster: ClusterApi,
    state: SharedLock<SelfEventSubscriberState>,
    failed_deliveries: SharedLock<u64>,
  ) -> Self {
    Self { subscriber, self_authority, cluster, state, failed_deliveries }
  }

  fn deliver(&mut self, cluster_event: &ClusterEvent) {
    let current_state = self.cluster.current_state();
    if let Some(self_up) = SelfUp::try_from_cluster_event(cluster_event, &self.self_authority, current_state)
      && self.subscriber.try_tell(self_up).is_err()
    {
      record_failed_delivery(&self.failed_deliveries);
    }
  }
}

impl EventStreamSubscriber for SelfUpSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { payload, .. } = event
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
      && let Some(seen_state) = update_seen_state(cluster_event, &self.self_authority)
    {
      self.state.with_write(|state| {
        state.seen_state = seen_state.clone();
      });
      if let Some(subscription_id) = claim_self_up_delivery(&self.state) {
        unsubscribe_completed_subscription(&self.cluster, subscription_id);
        self.deliver(cluster_event);
      } else if matches!(seen_state, SelfMemberSeenState::Removed(_)) {
        complete_self_event_subscription(&self.cluster, &self.state);
      }
    }
  }
}

struct SelfRemovedSubscriber {
  subscriber:        TypedActorRef<SelfRemoved>,
  self_authority:    String,
  cluster:           ClusterApi,
  state:             SharedLock<SelfEventSubscriberState>,
  failed_deliveries: SharedLock<u64>,
}

impl SelfRemovedSubscriber {
  fn new(
    subscriber: TypedActorRef<SelfRemoved>,
    self_authority: String,
    cluster: ClusterApi,
    state: SharedLock<SelfEventSubscriberState>,
    failed_deliveries: SharedLock<u64>,
  ) -> Self {
    Self { subscriber, self_authority, cluster, state, failed_deliveries }
  }

  fn deliver(&mut self, cluster_event: &ClusterEvent) {
    if let Some(self_removed) = SelfRemoved::try_from_cluster_event(cluster_event, &self.self_authority)
      && self.subscriber.try_tell(self_removed).is_err()
    {
      record_failed_delivery(&self.failed_deliveries);
    }
  }
}

impl EventStreamSubscriber for SelfRemovedSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    if let EventStreamEvent::Extension { payload, .. } = event
      && let Some(cluster_event) = payload.payload().downcast_ref::<ClusterEvent>()
      && let Some(seen_state) = update_seen_state(cluster_event, &self.self_authority)
    {
      self.state.with_write(|state| {
        state.seen_state = seen_state.clone();
      });
      if let Some((subscription_id, _self_removed)) = claim_self_removed_delivery(&self.state) {
        unsubscribe_completed_subscription(&self.cluster, subscription_id);
        self.deliver(cluster_event);
      }
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SelfMemberSeenState {
  BeforeUp,
  Up,
  Removed(SelfRemoved),
}

struct SelfEventSubscriberState {
  subscription_id: Option<u64>,
  replaying:       bool,
  completed:       bool,
  seen_state:      SelfMemberSeenState,
}

impl SelfEventSubscriberState {
  const fn new() -> Self {
    Self {
      subscription_id: None,
      replaying:       true,
      completed:       false,
      seen_state:      SelfMemberSeenState::BeforeUp,
    }
  }
}

fn update_seen_state(cluster_event: &ClusterEvent, self_authority: &str) -> Option<SelfMemberSeenState> {
  match cluster_event {
    | ClusterEvent::MemberStatusChanged { authority, to: NodeStatus::Up, .. } if authority == self_authority => {
      Some(SelfMemberSeenState::Up)
    },
    | ClusterEvent::MemberStatusChanged { authority, to: NodeStatus::Removed, .. } if authority == self_authority => {
      SelfRemoved::try_from_cluster_event(cluster_event, self_authority).map(SelfMemberSeenState::Removed)
    },
    | _ => None,
  }
}

fn seed_seen_state_from_current_state(
  state: &SharedLock<SelfEventSubscriberState>,
  self_authority: &str,
  current_state: &CurrentClusterState,
) {
  if let Some(record) = current_state.members.iter().find(|record| record.authority == self_authority)
    && record.status == NodeStatus::Up
  {
    state.with_write(|state| {
      if state.seen_state == SelfMemberSeenState::BeforeUp {
        state.seen_state = SelfMemberSeenState::Up;
      }
    });
  }
}

fn self_up_from_current_state(self_authority: &str, current_state: CurrentClusterState) -> Option<SelfUp> {
  let record = current_state
    .members
    .iter()
    .find(|record| record.authority == self_authority && record.status == NodeStatus::Up)?;
  Some(SelfUp::new(record.node_id.clone(), record.authority.clone(), current_state))
}

fn set_subscription_id(state: &SharedLock<SelfEventSubscriberState>, subscription_id: u64) -> bool {
  state.with_write(|state| {
    state.subscription_id = Some(subscription_id);
    state.replaying = false;
    state.completed
  })
}

fn claim_self_up_delivery(state: &SharedLock<SelfEventSubscriberState>) -> Option<Option<u64>> {
  state.with_write(|state| {
    if !state.replaying && !state.completed && state.seen_state == SelfMemberSeenState::Up {
      state.completed = true;
      Some(state.subscription_id)
    } else {
      None
    }
  })
}

fn claim_self_removed_delivery(state: &SharedLock<SelfEventSubscriberState>) -> Option<(Option<u64>, SelfRemoved)> {
  state.with_write(|state| match &state.seen_state {
    | SelfMemberSeenState::Removed(self_removed) if !state.replaying && !state.completed => {
      state.completed = true;
      Some((state.subscription_id, self_removed.clone()))
    },
    | SelfMemberSeenState::BeforeUp | SelfMemberSeenState::Up | SelfMemberSeenState::Removed(_) => None,
  })
}

fn complete_self_event_subscription(cluster: &ClusterApi, state: &SharedLock<SelfEventSubscriberState>) {
  let subscription_id = state.with_write(|state| {
    if state.completed {
      return None;
    }
    state.completed = true;
    state.subscription_id
  });
  unsubscribe_completed_subscription(cluster, subscription_id);
}

fn unsubscribe_completed_subscription(cluster: &ClusterApi, subscription_id: Option<u64>) {
  if let Some(subscription_id) = subscription_id {
    cluster.unsubscribe(subscription_id);
  }
}

fn record_failed_delivery(failed_deliveries: &SharedLock<u64>) {
  failed_deliveries.with_write(|count| *count += 1);
}

/// Typed facade for the kernel cluster API.
pub struct Cluster {
  inner: ClusterApi,
}

impl Cluster {
  /// Retrieves the typed cluster facade from a typed actor system.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster extension has not been installed.
  pub fn get<M>(system: &TypedActorSystem<M>) -> Result<Self, ClusterApiError>
  where
    M: Send + Sync + 'static, {
    ClusterApi::try_from_system(system.as_untyped()).map(Self::from_api)
  }

  const fn from_api(inner: ClusterApi) -> Self {
    Self { inner }
  }

  /// Returns the current cluster state snapshot.
  #[must_use]
  pub fn current_state(&self) -> CurrentClusterState {
    self.inner.current_state()
  }

  /// Subscribes a typed actor reference to cluster events.
  ///
  /// The returned subscription owns the kernel subscription identifier and can
  /// later be passed to the kernel event stream unsubscribe path.
  ///
  /// # Panics
  ///
  /// Panics when `event_types` is empty.
  #[must_use]
  pub fn subscribe(
    &self,
    subscriber: &TypedActorRef<ClusterEvent>,
    initial_state_mode: ClusterSubscriptionInitialStateMode,
    event_types: &[ClusterEventType],
  ) -> ClusterEventSubscription {
    let failed_deliveries = new_failed_delivery_counter();
    let subscriber = subscriber_handle(TypedClusterEventSubscriber::new(subscriber.clone(), failed_deliveries.clone()));
    let subscription = self.inner.subscribe(&subscriber, initial_state_mode, event_types);
    ClusterEventSubscription::new(subscription, failed_deliveries)
  }

  /// Subscribes a typed actor reference to local-member up events.
  #[must_use]
  pub fn subscribe_self_up(&self, subscriber: &TypedActorRef<SelfUp>) -> ClusterEventSubscription {
    let failed_deliveries = new_failed_delivery_counter();
    let state = new_self_event_subscriber_state();
    let self_authority = self.inner.self_authority();
    let mut typed_subscriber = subscriber.clone();
    let subscriber = subscriber_handle(SelfUpSubscriber::new(
      typed_subscriber.clone(),
      self_authority.clone(),
      self.inner.clone(),
      state.clone(),
      failed_deliveries.clone(),
    ));
    let subscription = self
      .inner
      .subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsEvents, &[ClusterEventType::MemberStatusChanged]);
    let current_state = self.inner.current_state();
    seed_seen_state_from_current_state(&state, &self_authority, &current_state);
    let completed = set_subscription_id(&state, subscription.id());
    if completed {
      self.inner.unsubscribe(subscription.id());
    }
    if let Some(self_up) = self_up_from_current_state(&self_authority, current_state)
      && let Some(subscription_id) = claim_self_up_delivery(&state)
    {
      unsubscribe_completed_subscription(&self.inner, subscription_id);
      if typed_subscriber.try_tell(self_up).is_err() {
        record_failed_delivery(&failed_deliveries);
      }
    }
    ClusterEventSubscription::new(subscription, failed_deliveries)
  }

  /// Subscribes a typed actor reference to local-member removed events.
  #[must_use]
  pub fn subscribe_self_removed(&self, subscriber: &TypedActorRef<SelfRemoved>) -> ClusterEventSubscription {
    let failed_deliveries = new_failed_delivery_counter();
    let state = new_self_event_subscriber_state();
    let self_authority = self.inner.self_authority();
    let typed_subscriber = subscriber.clone();
    let subscriber = subscriber_handle(SelfRemovedSubscriber::new(
      typed_subscriber.clone(),
      self_authority,
      self.inner.clone(),
      state.clone(),
      failed_deliveries.clone(),
    ));
    let subscription = self
      .inner
      .subscribe(&subscriber, ClusterSubscriptionInitialStateMode::AsEvents, &[ClusterEventType::MemberStatusChanged]);
    let completed = set_subscription_id(&state, subscription.id());
    if completed {
      self.inner.unsubscribe(subscription.id());
    }
    if let Some((subscription_id, self_removed)) = claim_self_removed_delivery(&state) {
      unsubscribe_completed_subscription(&self.inner, subscription_id);
      let mut typed_subscriber = typed_subscriber;
      if typed_subscriber.try_tell(self_removed).is_err() {
        record_failed_delivery(&failed_deliveries);
      }
    }
    ClusterEventSubscription::new(subscription, failed_deliveries)
  }

  /// Unsubscribes from cluster event delivery by subscription identifier.
  pub fn unsubscribe(&self, subscription_id: u64) {
    self.inner.unsubscribe(subscription_id);
  }

  pub(crate) fn join(&self, authority: &str) -> Result<(), ClusterError> {
    self.inner.join(authority)
  }

  pub(crate) fn leave(&self, authority: &str) -> Result<(), ClusterError> {
    self.inner.leave(authority)
  }

  pub(crate) fn down(&self, authority: &str) -> Result<(), ClusterError> {
    self.inner.down(authority)
  }

  pub(crate) fn prepare_for_full_cluster_shutdown(&self) -> Result<(), ClusterError> {
    self.inner.prepare_for_full_cluster_shutdown()
  }

  /// Builds a typed grain reference for the given typed identity.
  ///
  /// This is the fraktor equivalent of Pekko's `ClusterSharding#entityRefFor`.
  /// The caller must have already obtained this [`Cluster`] via [`Cluster::get`],
  /// which guarantees the cluster extension is installed. The returned
  /// [`GrainRef<M>`](crate::GrainRef) wraps a kernel reference constructed from
  /// the internal [`ClusterApi`] and the kernel identity derived from `identity`.
  ///
  /// # Note
  ///
  /// Obtaining a grain reference is infallible. Any failure (e.g. cluster
  /// extension not installed) occurs at [`Cluster::get`] time and is represented
  /// by [`ClusterApiError::ExtensionNotInstalled`].
  #[must_use]
  pub fn grain_ref_for<M>(&self, identity: &ClusterIdentity<M>) -> GrainRef<M>
  where
    M: Any + Send + Sync + 'static, {
    let kernel_ref = KernelGrainRef::new(self.inner.clone(), identity.as_kernel().clone());
    GrainRef::from_kernel(kernel_ref)
  }
}

fn new_failed_delivery_counter() -> SharedLock<u64> {
  SharedLock::new_with_driver::<DefaultMutex<_>>(0)
}

fn new_self_event_subscriber_state() -> SharedLock<SelfEventSubscriberState> {
  SharedLock::new_with_driver::<DefaultMutex<_>>(SelfEventSubscriberState::new())
}
