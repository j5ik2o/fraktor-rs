//! Opaque actor-ref sender lock cell.

use alloc::boxed::Box;
use core::{any::Any, mem::MaybeUninit};

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::actor::actor_ref::ActorRefSender;

type SenderReadFn = unsafe fn(*const (), &mut dyn FnMut(&Box<dyn ActorRefSender>));
type SenderWriteFn = unsafe fn(*const (), &mut dyn FnMut(&mut Box<dyn ActorRefSender>));

struct ClosureSenderLockCellState<R, W>
where
  R: Fn(&mut dyn FnMut(&Box<dyn ActorRefSender>)) + Send + Sync + 'static,
  W: Fn(&mut dyn FnMut(&mut Box<dyn ActorRefSender>)) + Send + Sync + 'static, {
  read:  R,
  write: W,
}

unsafe fn closure_sender_read<R, W>(state: *const (), f: &mut dyn FnMut(&Box<dyn ActorRefSender>))
where
  R: Fn(&mut dyn FnMut(&Box<dyn ActorRefSender>)) + Send + Sync + 'static,
  W: Fn(&mut dyn FnMut(&mut Box<dyn ActorRefSender>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureSenderLockCellState<R, W>>()) };
  (state.read)(f);
}

unsafe fn closure_sender_write<R, W>(state: *const (), f: &mut dyn FnMut(&mut Box<dyn ActorRefSender>))
where
  R: Fn(&mut dyn FnMut(&Box<dyn ActorRefSender>)) + Send + Sync + 'static,
  W: Fn(&mut dyn FnMut(&mut Box<dyn ActorRefSender>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureSenderLockCellState<R, W>>()) };
  (state.write)(f);
}

/// Opaque lock cell around `Box<dyn ActorRefSender>`.
pub struct SenderLockCell {
  keeper: ArcShared<dyn Any + Send + Sync>,
  state:  *const (),
  read:   SenderReadFn,
  write:  SenderWriteFn,
}

unsafe impl Send for SenderLockCell {}
unsafe impl Sync for SenderLockCell {}

impl SenderLockCell {
  /// Creates a new lock cell from read/write closure adapters.
  #[must_use]
  pub fn new<R, W>(read: R, write: W) -> Self
  where
    R: Fn(&mut dyn FnMut(&Box<dyn ActorRefSender>)) + Send + Sync + 'static,
    W: Fn(&mut dyn FnMut(&mut Box<dyn ActorRefSender>)) + Send + Sync + 'static, {
    let concrete = ArcShared::new(ClosureSenderLockCellState { read, write });
    let state = (&*concrete as *const ClosureSenderLockCellState<R, W>).cast::<()>();
    let keeper: ArcShared<dyn Any + Send + Sync> = concrete;
    Self { keeper, state, read: closure_sender_read::<R, W>, write: closure_sender_write::<R, W> }
  }

  /// Executes `f` under the read path of the underlying lock.
  pub fn with_read<R>(&self, f: impl FnOnce(&Box<dyn ActorRefSender>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.read)(self.state, &mut |sender| {
        if let Some(callback) = f.take() {
          result.write(callback(sender));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "sender read callback did not run");
    unsafe { result.assume_init() }
  }

  /// Executes `f` under the write path of the underlying lock.
  pub fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn ActorRefSender>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.write)(self.state, &mut |sender| {
        if let Some(callback) = f.take() {
          result.write(callback(sender));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "sender write callback did not run");
    unsafe { result.assume_init() }
  }
}

impl Clone for SenderLockCell {
  fn clone(&self) -> Self {
    Self { keeper: self.keeper.clone(), state: self.state, read: self.read, write: self.write }
  }
}
