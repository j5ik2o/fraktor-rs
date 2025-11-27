#![cfg(not(target_os = "none"))]

extern crate alloc;

use alloc::boxed::Box;
use core::{
  ffi::c_void,
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};
use std::{thread, time::Duration as StdDuration};

use fraktor_actor_rs::core::scheduler::{
  HardwareKind, HardwareTickDriver, Scheduler, SchedulerTickExecutor, TickDriver, TickDriverConfig, TickDriverError,
  TickDriverRuntime, TickExecutorSignal, TickFeed, TickFeedHandle, TickPulseHandler, TickPulseSource,
  TickPulseSourceShared,
};
use fraktor_utils_rs::core::{
  runtime_toolbox::{NoStdToolbox, ToolboxMutex},
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

const PULSE_PERIOD_NANOS: u64 = 10_000_000; // 10ms

/// Shared handler state for demo pulse source control.
type DemoPulseHandlerState = ArcShared<SpinSyncMutex<Option<HandlerSlot>>>;

/// Creates a demo pulse source with its control handle.
pub fn create_demo_pulse() -> (TickPulseSourceShared<NoStdToolbox>, DemoPulseHandle) {
  let handler = ArcShared::new(SpinSyncMutex::new(None));
  let enabled = ArcShared::new(AtomicBool::new(false));
  let handle = DemoPulseHandle { handler: handler.clone(), enabled: enabled.clone(), period: PULSE_PERIOD_NANOS };
  let source = DemoPulse::new(PULSE_PERIOD_NANOS, handler, enabled);
  let shared = HardwareTickDriver::<NoStdToolbox>::wrap_pulse(Box::new(source));
  (shared, handle)
}

/// Creates a hardware-based tick driver configuration for demos.
///
/// This is a convenience helper that wraps the builder configuration pattern,
/// combining a hardware tick driver with a scheduler executor.
/// Creates the demo pulse source internally.
pub fn hardware_tick_driver_config() -> TickDriverConfig<NoStdToolbox> {
  let (pulse, handle) = create_demo_pulse();
  hardware_tick_driver_config_with_pulse(pulse, handle)
}

/// Creates a hardware-based tick driver configuration with a custom pulse source.
///
/// Use this when you need more control over the pulse source.
pub fn hardware_tick_driver_config_with_pulse(
  pulse: TickPulseSourceShared<NoStdToolbox>,
  handle: DemoPulseHandle,
) -> TickDriverConfig<NoStdToolbox> {
  TickDriverConfig::new(move |ctx| {
    // Get resolution and capacity from SchedulerContext
    let scheduler: ArcShared<ToolboxMutex<Scheduler<NoStdToolbox>, NoStdToolbox>> = ctx.scheduler();
    let (resolution, capacity) = {
      let guard = scheduler.lock();
      let cfg = guard.config();
      (cfg.resolution(), cfg.profile().tick_buffer_quota())
    };

    // Create and start tick driver
    let driver = HardwareTickDriver::new(pulse.clone(), HardwareKind::Custom);
    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, capacity, signal);
    let driver_handle = driver.start(feed.clone())?;

    // Start scheduler executor
    let pump = StdTickDriverPump::spawn(handle.clone(), scheduler, feed.clone());

    // Create runtime with shutdown callback
    let runtime = TickDriverRuntime::new(driver_handle, feed).with_executor_shutdown(move || {
      drop(pump); // Drop will call stop()
    });

    Ok(runtime)
  })
}

/// Control handle for triggering and managing demo pulse callbacks.
#[derive(Clone)]
pub struct DemoPulseHandle {
  handler: DemoPulseHandlerState,
  enabled: ArcShared<AtomicBool>,
  period:  u64,
}

impl DemoPulseHandle {
  /// Fires the pulse callback if enabled.
  pub fn fire(&self) {
    if !self.enabled.load(Ordering::Acquire) {
      return;
    }
    if let Some(handler) = *self.handler.lock() {
      unsafe {
        (handler.func)(handler.ctx);
      }
    }
  }

  /// Returns the period in nanoseconds.
  pub fn period(&self) -> u64 {
    self.period
  }
}

struct DemoPulse {
  handler: DemoPulseHandlerState,
  enabled: ArcShared<AtomicBool>,
  period:  u64,
}

impl DemoPulse {
  fn new(period: u64, handler: DemoPulseHandlerState, enabled: ArcShared<AtomicBool>) -> Self {
    Self { handler, enabled, period }
  }
}

impl TickPulseSource for DemoPulse {
  fn enable(&mut self) -> Result<(), TickDriverError> {
    self.enabled.store(true, Ordering::Release);
    Ok(())
  }

  fn disable(&mut self) {
    self.enabled.store(false, Ordering::Release);
  }

  fn set_callback(&mut self, handler: TickPulseHandler) {
    *self.handler.lock() = Some(HandlerSlot::from(handler));
  }

  fn resolution(&self) -> Duration {
    Duration::from_nanos(self.period)
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
  running: ArcShared<AtomicBool>,
  handle:  Option<thread::JoinHandle<()>>,
}

impl StdTickDriverPump {
  pub fn spawn(pulse_handle: DemoPulseHandle, scheduler: SchedulerArc, feed: TickFeedHandle<NoStdToolbox>) -> Self {
    let running = ArcShared::new(AtomicBool::new(true));
    let signal = feed.signal();
    let sleep_interval = StdDuration::from_nanos(pulse_handle.period());
    let handle = thread::spawn({
      let running = running.clone();
      move || {
        let mut executor = SchedulerTickExecutor::new(scheduler, feed, signal);
        while running.load(Ordering::Acquire) {
          pulse_handle.fire();
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
