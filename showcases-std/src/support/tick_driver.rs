//! Tick driver support for std-based examples.
//!
//! Provides a hardware-based tick driver using `Arc<Mutex>` for shared state,
//! suitable for all std example programs.

#![allow(clippy::disallowed_types)]

use core::{
  ffi::c_void,
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};
use std::{
  boxed::Box,
  sync::{Arc, Mutex},
  thread,
  time::Duration as StdDuration,
};

#[cfg(feature = "advanced")]
use fraktor_actor_rs::core::kernel::scheduler::tick_driver::{
  AutoDriverMetadata, AutoProfileKind, TickDriverControl, TickDriverHandle, TickDriverKind, next_tick_driver_id,
};
use fraktor_actor_rs::core::kernel::scheduler::{
  SchedulerShared,
  tick_driver::{
    HardwareKind, HardwareTickDriver, SchedulerTickExecutor, TickDriver, TickDriverBundle, TickDriverConfig,
    TickDriverError, TickExecutorSignal, TickFeed, TickFeedHandle, TickPulseHandler, TickPulseSource,
  },
};
use fraktor_utils_rs::core::sync::SharedAccess;
#[cfg(feature = "advanced")]
use fraktor_utils_rs::core::sync::{ArcShared, RuntimeMutex};

const PULSE_PERIOD_NANOS: u64 = 10_000_000; // 10ms

/// Shared handler state for demo pulse source control.
type DemoPulseHandlerState = Arc<Mutex<Option<HandlerSlot>>>;

/// Creates a demo pulse handle that can control the pulse source.
///
/// The returned handle can be used to trigger pulses. The actual pulse source
/// should be created inside the tick driver configuration closure.
pub fn create_demo_pulse_handle() -> DemoPulseHandle {
  let handler = Arc::new(Mutex::new(None));
  let enabled = Arc::new(AtomicBool::new(false));
  DemoPulseHandle { handler, enabled, period: PULSE_PERIOD_NANOS }
}

/// Creates a hardware-based tick driver configuration for demos.
///
/// This is a convenience helper that wraps the builder configuration pattern,
/// combining a hardware tick driver with a scheduler executor.
/// Creates the demo pulse source internally.
pub fn hardware_tick_driver_config() -> (TickDriverConfig, DemoPulseHandle) {
  let handle = create_demo_pulse_handle();
  let config = hardware_tick_driver_config_with_handle(handle.clone());
  (config, handle)
}

/// Creates a hardware-based tick driver configuration with a custom pulse handle.
///
/// Use this when you need more control over the pulse source.
pub fn hardware_tick_driver_config_with_handle(handle: DemoPulseHandle) -> TickDriverConfig {
  TickDriverConfig::new(move |ctx| {
    let scheduler: SchedulerShared = ctx.scheduler();
    let (resolution, capacity) =
      scheduler.with_read(|s| (s.config().resolution(), s.config().profile().tick_buffer_quota()));

    let source = DemoPulse::new(PULSE_PERIOD_NANOS, handle.handler.clone(), handle.enabled.clone());

    let mut driver = HardwareTickDriver::new(Box::new(source), HardwareKind::Custom);
    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, capacity, signal);
    let driver_handle = driver.start(feed.clone())?;

    let pump = StdTickDriverPump::spawn(handle.clone(), scheduler, feed.clone());

    let bundle = TickDriverBundle::new(driver_handle, feed).with_executor_shutdown(move || {
      drop(pump);
    });

    Ok(bundle)
  })
}

/// Control handle for triggering and managing demo pulse callbacks.
#[derive(Clone)]
pub struct DemoPulseHandle {
  handler: DemoPulseHandlerState,
  enabled: Arc<AtomicBool>,
  period:  u64,
}

impl DemoPulseHandle {
  /// Fires the pulse callback if enabled.
  pub fn fire(&self) {
    if !self.enabled.load(Ordering::Acquire) {
      return;
    }
    if let Ok(guard) = self.handler.lock()
      && let Some(handler) = *guard
    {
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
  enabled: Arc<AtomicBool>,
  period:  u64,
}

impl DemoPulse {
  fn new(period: u64, handler: DemoPulseHandlerState, enabled: Arc<AtomicBool>) -> Self {
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
    if let Ok(mut guard) = self.handler.lock() {
      *guard = Some(HandlerSlot::from(handler));
    }
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

/// Drives the scheduler tick loop on a dedicated thread.
struct StdTickDriverPump {
  running: Arc<AtomicBool>,
  handle:  Option<thread::JoinHandle<()>>,
}

impl StdTickDriverPump {
  /// Spawns a new pump thread that periodically fires pulses and drives the scheduler.
  fn spawn(pulse_handle: DemoPulseHandle, scheduler: SchedulerShared, feed: TickFeedHandle) -> Self {
    let running = Arc::new(AtomicBool::new(true));
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
    if let Some(handle) = self.handle.take()
      && handle.join().is_err()
    {
      eprintln!("warn: tick driver thread panicked during join");
    }
  }
}

impl Drop for StdTickDriverPump {
  fn drop(&mut self) {
    self.stop();
  }
}

/// Creates a Tokio-based tick driver configuration with the default 10ms resolution.
///
/// # Panics
///
/// Panics if no Tokio runtime handle is available in the current context.
#[cfg(feature = "advanced")]
#[must_use]
pub fn tokio_tick_driver_config() -> TickDriverConfig {
  tokio_tick_driver_config_with_resolution(Duration::from_millis(10))
}

/// Creates a Tokio-based tick driver configuration with custom resolution.
///
/// # Panics
///
/// Panics if no Tokio runtime handle is available in the current context.
#[cfg(feature = "advanced")]
#[must_use]
pub fn tokio_tick_driver_config_with_resolution(resolution: Duration) -> TickDriverConfig {
  use tokio::{
    runtime::Handle,
    time::{MissedTickBehavior, interval},
  };

  TickDriverConfig::new(move |ctx| {
    #[allow(clippy::expect_used)]
    let handle = Handle::try_current().expect("Tokio runtime handle unavailable");

    let scheduler: SchedulerShared = ctx.scheduler();
    let capacity = scheduler.with_read(|s| s.config().profile().tick_buffer_quota());

    let signal = TickExecutorSignal::new();
    let feed = TickFeed::new(resolution, capacity, signal);
    let feed_clone = feed.clone();

    let tick_task = handle.spawn(async move {
      let mut ticker = interval(resolution);
      ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
      loop {
        ticker.tick().await;
        feed_clone.enqueue(1);
      }
    });

    let executor_feed = feed.clone();
    let executor_signal = executor_feed.signal();
    let executor_task = handle.spawn(async move {
      let mut executor = SchedulerTickExecutor::new(scheduler, executor_feed, executor_signal);
      loop {
        executor.drive_pending();
        tokio::time::sleep(resolution / 10).await;
      }
    });

    struct TokioQuickstartControl {
      tick_task:     tokio::task::JoinHandle<()>,
      executor_task: tokio::task::JoinHandle<()>,
    }
    impl TickDriverControl for TokioQuickstartControl {
      fn shutdown(&self) {
        self.tick_task.abort();
        self.executor_task.abort();
      }
    }

    let driver_id = next_tick_driver_id();
    let control: Box<dyn TickDriverControl> = Box::new(TokioQuickstartControl { tick_task, executor_task });
    let control = ArcShared::new(RuntimeMutex::new(control));
    let driver_handle = TickDriverHandle::new(driver_id, TickDriverKind::Auto, resolution, control);
    let metadata = AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id, resolution };

    Ok(TickDriverBundle::new(driver_handle, feed).with_auto_metadata(metadata))
  })
}
