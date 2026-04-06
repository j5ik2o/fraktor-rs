//! Tokio-based gossiper implementation.

#[cfg(test)]
mod tests;

use core::time::Duration;

use fraktor_actor_core_rs::core::kernel::event::stream::EventStreamShared;
use fraktor_utils_rs::core::time::TimerInstant;
use tokio::sync::oneshot;

use crate::{
  core::membership::{Gossiper, MembershipCoordinatorShared},
  std::{
    MembershipCoordinatorDriver, tokio_gossip_transport::TokioGossipTransport,
    tokio_gossiper_config::TokioGossiperConfig,
  },
};

/// Tokio-based gossiper.
pub struct TokioGossiper {
  config:       TokioGossiperConfig,
  coordinator:  MembershipCoordinatorShared,
  transport:    Option<TokioGossipTransport>,
  event_stream: EventStreamShared,
  runtime:      tokio::runtime::Handle,
  shutdown:     Option<oneshot::Sender<()>>,
  task:         Option<tokio::task::JoinHandle<()>>,
}

impl TokioGossiper {
  /// Creates a new Tokio gossiper.
  #[must_use]
  pub fn new(
    config: TokioGossiperConfig,
    coordinator: MembershipCoordinatorShared,
    transport: TokioGossipTransport,
    event_stream: EventStreamShared,
    runtime: tokio::runtime::Handle,
  ) -> Self {
    Self { config, coordinator, transport: Some(transport), event_stream, runtime, shutdown: None, task: None }
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

    let task = self.runtime.spawn(async move {
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
            if driver.poll(now).is_err() {
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
    let _ = shutdown.send(());
    if let Some(task) = self.task.take() {
      let _ = self.runtime.spawn(async move {
        let _ = task.await;
      });
    }
    Ok(())
  }
}
