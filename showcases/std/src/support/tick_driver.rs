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
use fraktor_actor_core_rs::core::kernel::actor::scheduler::tick_driver::{
  AutoDriverMetadata, AutoProfileKind, TickDriver, TickDriverControlShared, TickDriverHandle, TickDriverId,
  TickDriverKind, TickFeedHandle, next_tick_driver_id,
};
use fraktor_actor_core_rs::core::kernel::actor::scheduler::tick_driver::{
  HardwareKind, HardwareTickDriver, SchedulerTickExecutor, TickDriverConfig, TickDriverControl, TickDriverError,
  TickExecutorPump, TickPulseHandler, TickPulseSource,
};
#[cfg(feature = "advanced")]
use tokio::{
  runtime::Handle as TokioHandle,
  task::JoinHandle,
  time::{MissedTickBehavior, interval, sleep},
};

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
pub fn hardware_tick_driver_config() -> (TickDriverConfig, DemoPulseHandle) {
  let handle = create_demo_pulse_handle();
  let config = hardware_tick_driver_config_with_handle(handle.clone());
  (config, handle)
}

/// Creates a hardware-based tick driver configuration with a custom pulse handle.
pub fn hardware_tick_driver_config_with_handle(handle: DemoPulseHandle) -> TickDriverConfig {
  let source = DemoPulse::new(PULSE_PERIOD_NANOS, handle.handler.clone(), handle.enabled.clone());
  TickDriverConfig::runtime(
    Box::new(HardwareTickDriver::new(Box::new(source), HardwareKind::Custom)),
    Box::new(StdTickExecutorPump::new(handle)),
  )
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
struct StdTickExecutorPump {
  pulse_handle: DemoPulseHandle,
}

impl StdTickExecutorPump {
  const fn new(pulse_handle: DemoPulseHandle) -> Self {
    Self { pulse_handle }
  }
}

impl TickExecutorPump for StdTickExecutorPump {
  fn spawn(&mut self, mut executor: SchedulerTickExecutor) -> Result<Box<dyn TickDriverControl>, TickDriverError> {
    let running = Arc::new(AtomicBool::new(true));
    let sleep_interval = StdDuration::from_nanos(self.pulse_handle.period());
    let pulse_handle = self.pulse_handle.clone();
    let thread_handle = thread::spawn({
      let running = running.clone();
      move || {
        while running.load(Ordering::Acquire) {
          pulse_handle.fire();
          executor.drive_pending();
          thread::sleep(sleep_interval);
        }
      }
    });
    Ok(Box::new(StdTickExecutorControl { running, thread_handle: Mutex::new(Some(thread_handle)) }))
  }
}

struct StdTickExecutorControl {
  running:       Arc<AtomicBool>,
  thread_handle: Mutex<Option<thread::JoinHandle<()>>>,
}

impl TickDriverControl for StdTickExecutorControl {
  fn shutdown(&self) {
    self.running.store(false, Ordering::Release);
    if let Ok(mut handle) = self.thread_handle.lock()
      && let Some(handle) = handle.take()
      && handle.join().is_err()
    {
      eprintln!("warn: tick driver thread panicked during join");
    }
  }
}

/// Creates a Tokio-based tick driver configuration with the default 10ms resolution.
#[cfg(feature = "advanced")]
#[must_use]
pub fn tokio_tick_driver_config() -> TickDriverConfig {
  tokio_tick_driver_config_with_resolution(Duration::from_millis(10))
}

/// Creates a Tokio-based tick driver configuration with custom resolution.
#[cfg(feature = "advanced")]
#[must_use]
pub fn tokio_tick_driver_config_with_resolution(resolution: Duration) -> TickDriverConfig {
  TickDriverConfig::runtime(
    Box::new(TokioDemoTickDriver::new(resolution)),
    Box::new(TokioDemoTickExecutorPump::new(resolution)),
  )
}

#[cfg(feature = "advanced")]
struct TokioDemoTickDriver {
  id:         TickDriverId,
  resolution: Duration,
}

#[cfg(feature = "advanced")]
impl TokioDemoTickDriver {
  fn new(resolution: Duration) -> Self {
    Self { id: next_tick_driver_id(), resolution }
  }
}

#[cfg(feature = "advanced")]
impl TickDriver for TokioDemoTickDriver {
  fn id(&self) -> TickDriverId {
    self.id
  }

  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Auto
  }

  fn resolution(&self) -> Duration {
    self.resolution
  }

  fn start(&mut self, feed: TickFeedHandle) -> Result<TickDriverHandle, TickDriverError> {
    let handle = TokioHandle::try_current().map_err(|_| TickDriverError::HandleUnavailable)?;
    let resolution = self.resolution;
    let tick_task = handle.spawn(async move {
      let mut ticker = interval(resolution);
      ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
      loop {
        ticker.tick().await;
        feed.enqueue(1);
      }
    });

    let control: Box<dyn TickDriverControl> = Box::new(TokioDemoTickDriverControl { tick_task });
    let control = TickDriverControlShared::new(control);
    Ok(TickDriverHandle::new(self.id, TickDriverKind::Auto, resolution, control))
  }
}

#[cfg(feature = "advanced")]
struct TokioDemoTickDriverControl {
  tick_task: JoinHandle<()>,
}

#[cfg(feature = "advanced")]
impl TickDriverControl for TokioDemoTickDriverControl {
  fn shutdown(&self) {
    self.tick_task.abort();
  }
}

#[cfg(feature = "advanced")]
struct TokioDemoTickExecutorPump {
  resolution: Duration,
}

#[cfg(feature = "advanced")]
impl TokioDemoTickExecutorPump {
  const fn new(resolution: Duration) -> Self {
    Self { resolution }
  }
}

#[cfg(feature = "advanced")]
impl TickExecutorPump for TokioDemoTickExecutorPump {
  fn spawn(&mut self, mut executor: SchedulerTickExecutor) -> Result<Box<dyn TickDriverControl>, TickDriverError> {
    let handle = TokioHandle::try_current().map_err(|_| TickDriverError::HandleUnavailable)?;
    let resolution = self.resolution;
    let executor_task = handle.spawn(async move {
      loop {
        executor.drive_pending();
        sleep(resolution / 10).await;
      }
    });
    Ok(Box::new(TokioDemoTickExecutorControl { executor_task }))
  }

  fn auto_metadata(&self, driver_id: TickDriverId, resolution: Duration) -> Option<AutoDriverMetadata> {
    Some(AutoDriverMetadata { profile: AutoProfileKind::Tokio, driver_id, resolution })
  }
}

#[cfg(feature = "advanced")]
struct TokioDemoTickExecutorControl {
  executor_task: JoinHandle<()>,
}

#[cfg(feature = "advanced")]
impl TickDriverControl for TokioDemoTickExecutorControl {
  fn shutdown(&self) {
    self.executor_task.abort();
  }
}
