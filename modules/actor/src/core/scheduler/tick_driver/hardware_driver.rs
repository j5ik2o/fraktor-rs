//! Hardware-backed tick driver implementation.

use alloc::boxed::Box;
use core::{ffi::c_void, time::Duration};

use fraktor_utils_rs::core::{
  runtime_toolbox::{RuntimeToolbox, SyncMutexFamily, ToolboxMutex},
  sync::ArcShared,
};
use portable_atomic::{AtomicBool, Ordering};

use super::{
  HardwareKind, TickDriver, TickDriverControl, TickDriverError, TickDriverHandleGeneric, TickDriverId, TickDriverKind,
  TickFeedHandle, TickPulseHandler, TickPulseSource, next_tick_driver_id,
};

/// Tick driver that bridges hardware pulse sources into tick feeds.
///
/// # Interior Mutability Removed
///
/// This implementation no longer uses interior mutability. The `start` method
/// now requires `&mut self`. If shared access is needed, wrap in an external
/// synchronization primitive (e.g., `Mutex<HardwareTickDriver>`).
pub struct HardwareTickDriver {
  pulse: Box<dyn TickPulseSource>,
  kind:  HardwareKind,
  id:    TickDriverId,
}

impl HardwareTickDriver {
  /// Creates a new driver wrapping the provided pulse source.
  #[must_use]
  pub fn new(pulse: Box<dyn TickPulseSource>, kind: HardwareKind) -> Self {
    Self { pulse, kind, id: next_tick_driver_id() }
  }
}

impl<TB: RuntimeToolbox> TickDriver<TB> for HardwareTickDriver {
  fn id(&self) -> TickDriverId {
    self.id
  }

  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Hardware { source: self.kind }
  }

  fn resolution(&self) -> Duration {
    self.pulse.resolution()
  }

  fn start(&mut self, feed: TickFeedHandle<TB>) -> Result<TickDriverHandleGeneric<TB>, TickDriverError> {
    let context = Box::new(PulseContext { feed: feed.clone() });
    let ptr = Box::into_raw(context) as *mut c_void;
    let handler = TickPulseHandler { func: pulse_trampoline::<TB>, ctx: ptr };
    self.pulse.set_callback(handler);
    self.pulse.enable()?;
    let control = build_control::<TB>(ptr, feed);
    // Access fields directly to avoid trait method ambiguity with generic TB
    let id = self.id;
    let kind = TickDriverKind::Hardware { source: self.kind };
    let resolution = self.pulse.resolution();
    Ok(TickDriverHandleGeneric::new(id, kind, resolution, control))
  }
}

fn build_control<TB: RuntimeToolbox>(
  ctx: *mut c_void,
  feed: TickFeedHandle<TB>,
) -> ArcShared<ToolboxMutex<Box<dyn TickDriverControl>, TB>> {
  let control: Box<dyn TickDriverControl> = Box::new(HardwareDriverControl::new(ctx, feed));
  ArcShared::new(<TB::MutexFamily as SyncMutexFamily>::create(control))
}

struct PulseContext<TB: RuntimeToolbox> {
  feed: TickFeedHandle<TB>,
}

struct HardwareDriverControl<TB: RuntimeToolbox> {
  ctx:   *mut PulseContext<TB>,
  feed:  TickFeedHandle<TB>,
  freed: AtomicBool,
}

impl<TB: RuntimeToolbox> HardwareDriverControl<TB> {
  const fn new(ctx: *mut c_void, feed: TickFeedHandle<TB>) -> Self {
    Self { ctx: ctx as *mut PulseContext<TB>, feed, freed: AtomicBool::new(false) }
  }
}

impl<TB: RuntimeToolbox> TickDriverControl for HardwareDriverControl<TB> {
  fn shutdown(&self) {
    if !self.freed.swap(true, Ordering::AcqRel) && !self.ctx.is_null() {
      unsafe {
        drop(Box::from_raw(self.ctx));
      }
    }
    self.feed.mark_driver_inactive();
  }
}

unsafe extern "C" fn pulse_trampoline<TB: RuntimeToolbox>(ctx: *mut c_void) {
  if ctx.is_null() {
    return;
  }
  let feed = unsafe { &*(ctx as *mut PulseContext<TB>) }.feed.clone();
  feed.enqueue_from_isr(1);
}

unsafe impl<TB: RuntimeToolbox> Send for HardwareDriverControl<TB> {}
unsafe impl<TB: RuntimeToolbox> Sync for HardwareDriverControl<TB> {}
