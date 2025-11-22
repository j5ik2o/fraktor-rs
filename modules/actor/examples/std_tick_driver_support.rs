#![cfg(not(target_os = "none"))]
#![allow(clippy::disallowed_types)]
#![allow(clippy::collapsible_if)]

use core::{
  ffi::c_void,
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};
use std::{thread, time::Duration as StdDuration};

use fraktor_actor_rs::core::scheduler::{
  HardwareKind, HardwareTickDriver, Scheduler, SchedulerTickExecutor, TickDriver, TickDriverConfig, TickDriverError,
  TickDriverRuntime, TickExecutorSignal, TickFeed, TickFeedHandle, TickPulseHandler, TickPulseSource,
};
use fraktor_utils_rs::{
  core::{runtime_toolbox::ToolboxMutex, sync::ArcShared},
  std::runtime_toolbox::StdToolbox,
};

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
pub fn hardware_tick_driver_config() -> TickDriverConfig<StdToolbox> {
  TickDriverConfig::new(|ctx| {
    // Get resolution and capacity from SchedulerContext
    let scheduler: ArcShared<ToolboxMutex<Scheduler<StdToolbox>, StdToolbox>> = ctx.scheduler();
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
  handler: std::sync::Mutex<Option<HandlerSlot>>,
  enabled: AtomicBool,
  period:  u64,
}

impl DemoPulse {
  const fn new(period: u64) -> Self {
    Self { handler: std::sync::Mutex::new(None), enabled: AtomicBool::new(false), period }
  }

  fn fire(&self) {
    if !self.enabled.load(Ordering::Acquire) {
      return;
    }
    if let Ok(guard) = self.handler.lock() {
      if let Some(handler) = *guard {
        unsafe {
          (handler.func)(handler.ctx);
        }
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
    if let Ok(mut guard) = self.handler.lock() {
      *guard = Some(HandlerSlot::from(handler));
    }
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

type SchedulerArc = ArcShared<ToolboxMutex<Scheduler<StdToolbox>, StdToolbox>>;

pub struct StdTickDriverPump {
  running: ArcShared<AtomicBool>,
  handle:  Option<thread::JoinHandle<()>>,
}

impl StdTickDriverPump {
  pub fn spawn(pulse: &'static DemoPulse, scheduler: SchedulerArc, feed: TickFeedHandle<StdToolbox>) -> Self {
    let running = ArcShared::new(AtomicBool::new(true));
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
