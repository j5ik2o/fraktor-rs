//! Hardware-backed tick driver implementation.

use alloc::boxed::Box;
use core::{ffi::c_void, time::Duration};

use portable_atomic::{AtomicBool, Ordering};

use super::{
  HardwareKind, TickDriver, TickDriverControl, TickDriverControlShared, TickDriverError, TickDriverHandle,
  TickDriverId, TickDriverKind, TickFeedHandle, TickPulseHandler, TickPulseSource, tick_driver_trait::next_tick_driver_id,
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

impl TickDriver for HardwareTickDriver {
  fn id(&self) -> TickDriverId {
    self.id
  }

  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Hardware { source: self.kind }
  }

  fn resolution(&self) -> Duration {
    self.pulse.resolution()
  }

  fn start(
    &mut self,
    feed: TickFeedHandle,
  ) -> Result<TickDriverHandle, TickDriverError> {
    let context = Box::new(PulseContext { feed: feed.clone() });
    let ptr = Box::into_raw(context) as *mut c_void;
    let handler = TickPulseHandler { func: pulse_trampoline, ctx: ptr };
    self.pulse.set_callback(handler);
    self.pulse.enable()?;
    let control = build_control(ptr, feed);
    // Access fields directly to avoid trait method ambiguity.
    let id = self.id;
    let kind = TickDriverKind::Hardware { source: self.kind };
    let resolution = self.pulse.resolution();
    Ok(TickDriverHandle::new(id, kind, resolution, control))
  }
}

fn build_control(ctx: *mut c_void, feed: TickFeedHandle) -> TickDriverControlShared {
  let control: Box<dyn TickDriverControl> = Box::new(HardwareDriverControl::new(ctx, feed));
  TickDriverControlShared::new(control)
}

struct PulseContext {
  feed: TickFeedHandle,
}

struct HardwareDriverControl {
  ctx:   *mut PulseContext,
  feed:  TickFeedHandle,
  freed: AtomicBool,
}

impl HardwareDriverControl {
  const fn new(ctx: *mut c_void, feed: TickFeedHandle) -> Self {
    Self { ctx: ctx as *mut PulseContext, feed, freed: AtomicBool::new(false) }
  }
}

impl TickDriverControl for HardwareDriverControl {
  fn shutdown(&self) {
    if !self.freed.swap(true, Ordering::AcqRel) && !self.ctx.is_null() {
      unsafe {
        drop(Box::from_raw(self.ctx));
      }
    }
    self.feed.mark_driver_inactive();
  }
}

unsafe extern "C" fn pulse_trampoline(ctx: *mut c_void) {
  if ctx.is_null() {
    return;
  }
  let feed = unsafe { &*(ctx as *mut PulseContext) }.feed.clone();
  feed.enqueue_from_isr(1);
}

unsafe impl Send for HardwareDriverControl {}
unsafe impl Sync for HardwareDriverControl {}
