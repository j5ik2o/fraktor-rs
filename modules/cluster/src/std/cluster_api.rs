//! Std wrapper for the cluster API.

use core::time::Duration;

use fraktor_actor_rs::std::{
  actor::ActorRef,
  futures::ActorFutureShared,
  messaging::{AnyMessage, AskResponse, AskResult},
  system::ActorSystem,
};
use fraktor_utils_rs::std::runtime_toolbox::StdToolbox;

use crate::core::{
  ClusterApiError, ClusterApiGeneric, ClusterError, ClusterRequestError, ClusterResolveError, identity::ClusterIdentity,
};

/// Cluster API facade bound to a std actor system.
pub struct ClusterApi {
  inner: ClusterApiGeneric<StdToolbox>,
}

impl ClusterApi {
  /// Retrieves the cluster API from a std actor system.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster extension has not been installed.
  pub fn try_from_system(system: &ActorSystem) -> Result<Self, ClusterApiError> {
    ClusterApiGeneric::try_from_system(system.as_core()).map(Self::from_core)
  }

  /// Creates a wrapper from the core cluster API.
  #[must_use]
  pub const fn from_core(inner: ClusterApiGeneric<StdToolbox>) -> Self {
    Self { inner }
  }

  /// Borrows the underlying core cluster API.
  #[must_use]
  pub const fn as_core(&self) -> &ClusterApiGeneric<StdToolbox> {
    &self.inner
  }

  /// Consumes the wrapper and returns the core cluster API.
  #[must_use]
  pub fn into_core(self) -> ClusterApiGeneric<StdToolbox> {
    self.inner
  }

  /// Resolves an identity into an actor reference.
  ///
  /// # Errors
  ///
  /// Returns an error if the cluster is not started, the kind is not registered,
  /// PID lookup fails, or actor resolution fails.
  pub fn get(&self, identity: &ClusterIdentity) -> Result<ActorRef, ClusterResolveError> {
    self.inner.get(identity)
  }

  /// Sends a request and returns the ask response handle.
  ///
  /// # Errors
  ///
  /// Returns an error if resolution fails, sending fails, or timeout scheduling fails.
  pub fn request(
    &self,
    identity: &ClusterIdentity,
    message: AnyMessage,
    timeout: Option<Duration>,
  ) -> Result<AskResponse, ClusterRequestError> {
    self.inner.request(identity, message, timeout)
  }

  /// Sends a request and returns the shared response future.
  ///
  /// # Errors
  ///
  /// Returns an error if resolution fails, sending fails, or timeout scheduling fails.
  pub fn request_future(
    &self,
    identity: &ClusterIdentity,
    message: AnyMessage,
    timeout: Option<Duration>,
  ) -> Result<ActorFutureShared<AskResult>, ClusterRequestError> {
    self.inner.request_future(identity, message, timeout)
  }

  /// Explicitly downs the provided member authority.
  ///
  /// # Errors
  ///
  /// Returns an error when the cluster is not started or downing fails.
  pub fn down(&self, authority: &str) -> Result<(), ClusterError> {
    self.inner.down(authority)
  }
}
