use std::{sync::Mutex, time::Duration};

use fraktor_actor_core_rs::scheduler::{
  TickDriver, TickDriverControl, TickDriverError, TickDriverFactory, TickDriverHandle, TickDriverId, TickDriverKind,
  TickFeedHandle, next_tick_driver_id,
};
use fraktor_utils_core_rs::sync::ArcShared;
use fraktor_utils_std_rs::runtime_toolbox::StdToolbox;
use tokio::{
  runtime::Handle,
  task::JoinHandle,
  time::{MissedTickBehavior, interval},
};

/// Factory producing Tokio interval-based drivers.
pub(super) struct TokioIntervalDriverFactory {
  handle:     Handle,
  resolution: Duration,
}

impl TokioIntervalDriverFactory {
  pub(super) fn new(handle: Handle, resolution: Duration) -> Self {
    Self { handle, resolution }
  }
}

impl TickDriverFactory<StdToolbox> for TokioIntervalDriverFactory {
  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn resolution(&self) -> Duration {
    self.resolution
  }

  fn build(&self) -> Result<Box<dyn TickDriver<StdToolbox>>, TickDriverError> {
    Ok(Box::new(TokioIntervalDriver {
      id:         next_tick_driver_id(),
      handle:     self.handle.clone(),
      resolution: self.resolution,
    }))
  }
}

struct TokioIntervalDriver {
  id:         TickDriverId,
  handle:     Handle,
  resolution: Duration,
}

impl TickDriver<StdToolbox> for TokioIntervalDriver {
  fn id(&self) -> TickDriverId {
    self.id
  }

  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn resolution(&self) -> Duration {
    self.resolution
  }

  fn start(&self, feed: TickFeedHandle<StdToolbox>) -> Result<TickDriverHandle, TickDriverError> {
    let mut ticker = interval(self.resolution);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let join = self.handle.spawn(async move {
      let mut ticker = ticker;
      loop {
        ticker.tick().await;
        feed.enqueue(1);
      }
    });
    let control = ArcShared::new(TokioIntervalDriverControl::new(join));
    Ok(TickDriverHandle::new(self.id, self.kind(), self.resolution, control))
  }
}

struct TokioIntervalDriverControl {
  join: Mutex<Option<JoinHandle<()>>>,
}

impl TokioIntervalDriverControl {
  fn new(join: JoinHandle<()>) -> Self {
    Self { join: Mutex::new(Some(join)) }
  }
}

impl TickDriverControl for TokioIntervalDriverControl {
  fn shutdown(&self) {
    if let Some(handle) = self.join.lock().expect("lock").take() {
      handle.abort();
    }
  }
}
