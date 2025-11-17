//! Hardware-backed tick driver implementation.

use alloc::boxed::Box;
use core::{ffi::c_void, marker::PhantomData, time::Duration};

use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  sync::{ArcShared, sync_mutex_like::SpinSyncMutex},
};

use super::{
  HardwareKind, TickDriver, TickDriverControl, TickDriverError, TickDriverHandle, TickDriverId, TickDriverKind,
  TickFeedHandle, TickPulseHandler, TickPulseSource, next_tick_driver_id,
};

/// Tick driver that bridges hardware pulse sources into tick feeds.
pub struct HardwareTickDriver<TB: RuntimeToolbox> {
  pulse: &'static dyn TickPulseSource,
  kind:  HardwareKind,
  id:    TickDriverId,
  _pd:   PhantomData<TB>,
}

impl<TB: RuntimeToolbox> HardwareTickDriver<TB> {
  /// Creates a new driver wrapping the provided pulse source.
  #[must_use]
  pub fn new(pulse: &'static dyn TickPulseSource, kind: HardwareKind) -> Self {
    Self { pulse, kind, id: next_tick_driver_id(), _pd: PhantomData }
  }

  fn build_control(&self, ctx: *mut c_void, feed: TickFeedHandle<TB>) -> ArcShared<HardwareDriverControl<TB>> {
    ArcShared::new(HardwareDriverControl::new(self.pulse, ctx, feed))
  }
}

impl<TB: RuntimeToolbox> TickDriver<TB> for HardwareTickDriver<TB> {
  fn id(&self) -> TickDriverId {
    self.id
  }

  fn kind(&self) -> TickDriverKind {
    TickDriverKind::Hardware { source: self.kind }
  }

  fn resolution(&self) -> Duration {
    self.pulse.resolution()
  }

  fn start(&self, feed: TickFeedHandle<TB>) -> Result<TickDriverHandle, TickDriverError> {
    let context = Box::new(PulseContext { feed: feed.clone() });
    let ptr = Box::into_raw(context) as *mut c_void;
    let handler = TickPulseHandler { func: pulse_trampoline::<TB>, ctx: ptr };
    self.pulse.set_callback(handler);
    self.pulse.enable()?;
    let control = self.build_control(ptr, feed);
    Ok(TickDriverHandle::new(self.id(), self.kind(), self.resolution(), control))
  }
}

struct PulseContext<TB: RuntimeToolbox> {
  feed: TickFeedHandle<TB>,
}

struct HardwareDriverControl<TB: RuntimeToolbox> {
  pulse: &'static dyn TickPulseSource,
  ctx:   SpinSyncMutex<Option<*mut PulseContext<TB>>>,
  feed:  TickFeedHandle<TB>,
}

impl<TB: RuntimeToolbox> HardwareDriverControl<TB> {
  fn new(pulse: &'static dyn TickPulseSource, ctx: *mut c_void, feed: TickFeedHandle<TB>) -> Self {
    Self { pulse, ctx: SpinSyncMutex::new(Some(ctx as *mut PulseContext<TB>)), feed }
  }
}

impl<TB: RuntimeToolbox> TickDriverControl for HardwareDriverControl<TB> {
  fn shutdown(&self) {
    self.pulse.disable();
    if let Some(ptr) = self.ctx.lock().take() {
      unsafe {
        drop(Box::from_raw(ptr));
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
