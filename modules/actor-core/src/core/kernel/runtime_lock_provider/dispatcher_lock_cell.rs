//! Opaque dispatcher lock cell.

use alloc::boxed::Box;
use core::{any::Any, mem::MaybeUninit};

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::dispatch::dispatcher::MessageDispatcher;

type DispatcherReadFn = unsafe fn(*const (), &mut dyn FnMut(&Box<dyn MessageDispatcher>));
type DispatcherWriteFn = unsafe fn(*const (), &mut dyn FnMut(&mut Box<dyn MessageDispatcher>));

struct ClosureDispatcherLockCellState<R, W>
where
  R: Fn(&mut dyn FnMut(&Box<dyn MessageDispatcher>)) + Send + Sync + 'static,
  W: Fn(&mut dyn FnMut(&mut Box<dyn MessageDispatcher>)) + Send + Sync + 'static, {
  read:  R,
  write: W,
}

unsafe fn closure_dispatcher_read<R, W>(state: *const (), f: &mut dyn FnMut(&Box<dyn MessageDispatcher>))
where
  R: Fn(&mut dyn FnMut(&Box<dyn MessageDispatcher>)) + Send + Sync + 'static,
  W: Fn(&mut dyn FnMut(&mut Box<dyn MessageDispatcher>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureDispatcherLockCellState<R, W>>()) };
  (state.read)(f);
}

unsafe fn closure_dispatcher_write<R, W>(state: *const (), f: &mut dyn FnMut(&mut Box<dyn MessageDispatcher>))
where
  R: Fn(&mut dyn FnMut(&Box<dyn MessageDispatcher>)) + Send + Sync + 'static,
  W: Fn(&mut dyn FnMut(&mut Box<dyn MessageDispatcher>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureDispatcherLockCellState<R, W>>()) };
  (state.write)(f);
}

/// Opaque lock cell around `Box<dyn MessageDispatcher>`.
pub struct DispatcherLockCell {
  keeper: ArcShared<dyn Any + Send + Sync>,
  state:  *const (),
  read:   DispatcherReadFn,
  write:  DispatcherWriteFn,
}

unsafe impl Send for DispatcherLockCell {}
unsafe impl Sync for DispatcherLockCell {}

impl DispatcherLockCell {
  /// Creates a new lock cell from read/write closure adapters.
  #[must_use]
  pub fn new<R, W>(read: R, write: W) -> Self
  where
    R: Fn(&mut dyn FnMut(&Box<dyn MessageDispatcher>)) + Send + Sync + 'static,
    W: Fn(&mut dyn FnMut(&mut Box<dyn MessageDispatcher>)) + Send + Sync + 'static, {
    let concrete = ArcShared::new(ClosureDispatcherLockCellState { read, write });
    let state = (&*concrete as *const ClosureDispatcherLockCellState<R, W>).cast::<()>();
    let keeper: ArcShared<dyn Any + Send + Sync> = concrete;
    Self { keeper, state, read: closure_dispatcher_read::<R, W>, write: closure_dispatcher_write::<R, W> }
  }

  /// Executes `f` under the read path of the underlying lock.
  pub fn with_read<R>(&self, f: impl FnOnce(&Box<dyn MessageDispatcher>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.read)(self.state, &mut |dispatcher| {
        if let Some(callback) = f.take() {
          result.write(callback(dispatcher));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "dispatcher read callback did not run");
    unsafe { result.assume_init() }
  }

  /// Executes `f` under the write path of the underlying lock.
  pub fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn MessageDispatcher>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.write)(self.state, &mut |dispatcher| {
        if let Some(callback) = f.take() {
          result.write(callback(dispatcher));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "dispatcher write callback did not run");
    unsafe { result.assume_init() }
  }
}

impl Clone for DispatcherLockCell {
  fn clone(&self) -> Self {
    Self { keeper: self.keeper.clone(), state: self.state, read: self.read, write: self.write }
  }
}
