#![cfg(not(target_os = "none"))]

use alloc::sync::Arc;
use core::{
  ffi::c_void,
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};
use std::{thread, time::Duration as StdDuration};

use fraktor_actor_core_rs::{
  NoStdToolbox, ToolboxMutex,
  scheduler::{
    HardwareKind, HardwareTickDriver, Scheduler, SchedulerTickExecutor, TickDriver, TickDriverConfig, TickDriverError,
    TickDriverRuntime, TickExecutorSignal, TickFeed, TickFeedHandle, TickPulseHandler, TickPulseSource,
  },
};
use fraktor_utils_core_rs::core::sync::{ArcShared, sync_mutex_like::SpinSyncMutex};

const PULSE_PERIOD_NANOS: u64 = 10_000_000; // 10ms
static DEMO_PULSE: DemoPulse = DemoPulse::new(PULSE_PERIOD_NANOS);

/// Returns the demo pulse source used by the examples.
pub fn demo_pulse() -> &'static DemoPulse {
  &DEMO_PULSE
}

/// Creates a hardware-based tick driver configuration for demos.
///
/// This is a convenience helper that wraps the builder configuration pattern,
/// combining a hardware tick driver with a scheduler executor.
pub fn hardware_tick_driver_config() -> TickDriverConfig<NoStdToolbox> {
  TickDriverConfig::new(|ctx| {
    // Get resolution and capacity from SchedulerContext
    let scheduler: ArcShared<ToolboxMutex<Scheduler<NoStdToolbox>, NoStdToolbox>> = ctx.scheduler();
    let (resolution, capacity) = {
      let guard = scheduler.lock();
      let cfg = guard.config();
      (cfg.resolution(), cfg.profile().tick_buffer_quota())
    };

    // Create and start tick driver
    let driver = HardwareTickDriver::new(demo_pulse(), HardwareKind::Custom);
    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, capacity, signal);
    let handle = driver.start(feed.clone())?;

    // Start scheduler executor
    let pump = StdTickDriverPump::spawn(demo_pulse(), scheduler, feed.clone());

    // Create runtime with shutdown callback
    let runtime = TickDriverRuntime::new(handle, feed).with_executor_shutdown(move || {
      drop(pump); // Drop will call stop()
    });

    Ok(runtime)
  })
}

pub struct DemoPulse {
  handler: SpinSyncMutex<Option<HandlerSlot>>,
  enabled: AtomicBool,
  period:  u64,
}

impl DemoPulse {
  const fn new(period: u64) -> Self {
    Self { handler: SpinSyncMutex::new(None), enabled: AtomicBool::new(false), period }
  }

  fn fire(&self) {
    if !self.enabled.load(Ordering::Acquire) {
      return;
    }
    if let Some(handler) = *self.handler.lock() {
      unsafe {
        (handler.func)(handler.ctx);
      }
    }
  }

  fn resolution(&self) -> Duration {
    Duration::from_nanos(self.period)
  }
}

impl TickPulseSource for DemoPulse {
  fn enable(&self) -> Result<(), TickDriverError> {
    self.enabled.store(true, Ordering::Release);
    Ok(())
  }

  fn disable(&self) {
    self.enabled.store(false, Ordering::Release);
  }

  fn set_callback(&self, handler: TickPulseHandler) {
    *self.handler.lock() = Some(HandlerSlot::from(handler));
  }

  fn resolution(&self) -> Duration {
    self.resolution()
  }
}

#[derive(Clone, Copy)]
struct HandlerSlot {
  func: unsafe extern "C" fn(*mut c_void),
  ctx:  *mut c_void,
}

impl HandlerSlot {
  const fn from(handler: TickPulseHandler) -> Self {
    Self { func: handler.func, ctx: handler.ctx }
  }
}

unsafe impl Send for HandlerSlot {}
unsafe impl Sync for HandlerSlot {}

type SchedulerArc = ArcShared<ToolboxMutex<Scheduler<NoStdToolbox>, NoStdToolbox>>;

pub struct StdTickDriverPump {
  running: Arc<AtomicBool>,
  handle:  Option<thread::JoinHandle<()>>,
}

impl StdTickDriverPump {
  pub fn spawn(pulse: &'static DemoPulse, scheduler: SchedulerArc, feed: TickFeedHandle<NoStdToolbox>) -> Self {
    let running = Arc::new(AtomicBool::new(true));
    let signal = feed.signal();
    let sleep_interval = StdDuration::from_nanos(pulse.period);
    let handle = thread::spawn({
      let running = running.clone();
      move || {
        let mut executor = SchedulerTickExecutor::new(scheduler, feed, signal);
        while running.load(Ordering::Acquire) {
          pulse.fire();
          executor.drive_pending();
          thread::sleep(sleep_interval);
        }
      }
    });
    Self { running, handle: Some(handle) }
  }

  fn stop(&mut self) {
    self.running.store(false, Ordering::Release);
    if let Some(handle) = self.handle.take() {
      let _ = handle.join();
    }
  }
}

impl Drop for StdTickDriverPump {
  fn drop(&mut self) {
    self.stop();
  }
}
