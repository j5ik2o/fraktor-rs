//! Tokio-based gossiper implementation.

use core::time::Duration;

use fraktor_actor_core_kernel_rs::event::stream::EventStreamShared;
use fraktor_cluster_core_kernel_rs::{
  extension::ClusterProviderShared,
  membership::{Gossiper, MembershipCoordinatorError, MembershipCoordinatorShared},
};
use fraktor_utils_core_rs::time::TimerInstant;
use tokio::{
  runtime::Handle,
  sync::oneshot::{self, Sender},
  task::JoinHandle,
};

use super::{
  membership_coordinator_driver::MembershipCoordinatorDriver, tokio_gossip_transport::TokioGossipTransport,
  tokio_gossiper_config::TokioGossiperConfig,
};
use crate::cluster_provider::StdSplitBrainResolverProvider;

#[cfg(test)]
#[path = "tokio_gossiper_test.rs"]
mod tests;

/// Tokio-based gossiper.
pub struct TokioGossiper {
  config:       TokioGossiperConfig,
  coordinator:  MembershipCoordinatorShared,
  transport:    Option<TokioGossipTransport>,
  event_stream: EventStreamShared,
  tokio_handle: Handle,
  shutdown:     Option<Sender<()>>,
  task:         Option<JoinHandle<()>>,
  sbr_downing:  Option<(StdSplitBrainResolverProvider, String, ClusterProviderShared)>,
}

impl TokioGossiper {
  /// Creates a new Tokio gossiper.
  #[must_use]
  pub fn new(
    config: TokioGossiperConfig,
    coordinator: MembershipCoordinatorShared,
    transport: TokioGossipTransport,
    event_stream: EventStreamShared,
    tokio_handle: Handle,
  ) -> Self {
    Self {
      config,
      coordinator,
      transport: Some(transport),
      event_stream,
      tokio_handle,
      shutdown: None,
      task: None,
      sbr_downing: None,
    }
  }

  /// Enables automatic Split Brain Resolver down execution during membership polling.
  #[must_use]
  pub fn with_split_brain_resolver_downing(
    mut self,
    provider: StdSplitBrainResolverProvider,
    local_authority: impl Into<String>,
    cluster_provider: ClusterProviderShared,
  ) -> Self {
    self.sbr_downing = Some((provider, local_authority.into(), cluster_provider));
    self
  }

  /// Returns the shared coordinator handle.
  #[must_use]
  pub const fn coordinator(&self) -> &MembershipCoordinatorShared {
    &self.coordinator
  }
}

impl Gossiper for TokioGossiper {
  fn start(&mut self) -> Result<(), &'static str> {
    if self.task.is_some() {
      return Err("already started");
    }
    if self.config.tick_interval == Duration::from_millis(0) {
      return Err("tick_interval must be > 0");
    }
    let transport = self.transport.take().ok_or("transport missing")?;
    let coordinator = self.coordinator.clone();
    let event_stream = self.event_stream.clone();
    let tick_resolution = self.config.tick_resolution;
    let tick_interval = self.config.tick_interval;
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let mut driver = MembershipCoordinatorDriver::new(coordinator, transport, event_stream);
    if let Some((provider, local_authority, cluster_provider)) = self.sbr_downing.take() {
      driver = driver.with_split_brain_resolver_downing(provider, local_authority, cluster_provider);
    }

    let task = self.tokio_handle.spawn(async move {
      let mut interval = tokio::time::interval(tick_interval);
      let mut ticks: u64 = 0;
      loop {
        tokio::select! {
          _ = &mut shutdown_rx => {
            break;
          }
          _ = interval.tick() => {
            ticks = ticks.saturating_add(1);
            let now = TimerInstant::from_ticks(ticks, tick_resolution);
            if driver.handle_gossip_deltas(now).is_err() {
              break;
            }
            if let Err(error) = driver.poll(now) {
              if should_continue_after_poll_error(&error) {
                tracing::warn!(?error, "membership coordinator poll error did not stop gossip");
                continue;
              }
              tracing::warn!(?error, "membership coordinator poll failed");
              break;
            }
          }
        }
      }
    });
    self.shutdown = Some(shutdown_tx);
    self.task = Some(task);
    Ok(())
  }

  fn stop(&mut self) -> Result<(), &'static str> {
    let shutdown = self.shutdown.take().ok_or("not started")?;
    if shutdown.send(()).is_err() {
      tracing::debug!("gossip shutdown receiver already closed");
    }
    if let Some(task) = self.task.take() {
      // spawn は JoinHandle を返すが、join 不要の fire-and-forget shutdown 経路のため破棄する。
      drop(self.tokio_handle.spawn(async move {
        if let Err(err) = task.await {
          tracing::debug!("gossip task join failed during shutdown: {err}");
        }
      }));
    }
    Ok(())
  }
}

fn should_continue_after_poll_error(error: &MembershipCoordinatorError) -> bool {
  matches!(error, MembershipCoordinatorError::ClusterProvider(_))
}
