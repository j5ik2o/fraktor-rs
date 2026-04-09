//! Opaque executor lock cell.

use alloc::boxed::Box;
use core::{any::Any, mem::MaybeUninit};

use fraktor_utils_core_rs::core::sync::ArcShared;

use crate::core::kernel::dispatch::dispatcher::Executor;

type ExecutorReadFn = unsafe fn(*const (), &mut dyn FnMut(&Box<dyn Executor>));
type ExecutorWriteFn = unsafe fn(*const (), &mut dyn FnMut(&mut Box<dyn Executor>));

struct ClosureExecutorLockCellState<R, W>
where
  R: Fn(&mut dyn FnMut(&Box<dyn Executor>)) + Send + Sync + 'static,
  W: Fn(&mut dyn FnMut(&mut Box<dyn Executor>)) + Send + Sync + 'static, {
  read:  R,
  write: W,
}

unsafe fn closure_executor_read<R, W>(state: *const (), f: &mut dyn FnMut(&Box<dyn Executor>))
where
  R: Fn(&mut dyn FnMut(&Box<dyn Executor>)) + Send + Sync + 'static,
  W: Fn(&mut dyn FnMut(&mut Box<dyn Executor>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureExecutorLockCellState<R, W>>()) };
  (state.read)(f);
}

unsafe fn closure_executor_write<R, W>(state: *const (), f: &mut dyn FnMut(&mut Box<dyn Executor>))
where
  R: Fn(&mut dyn FnMut(&Box<dyn Executor>)) + Send + Sync + 'static,
  W: Fn(&mut dyn FnMut(&mut Box<dyn Executor>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureExecutorLockCellState<R, W>>()) };
  (state.write)(f);
}

/// Opaque lock cell around `Box<dyn Executor>`.
pub struct ExecutorLockCell {
  keeper: ArcShared<dyn Any + Send + Sync>,
  state:  *const (),
  read:   ExecutorReadFn,
  write:  ExecutorWriteFn,
}

unsafe impl Send for ExecutorLockCell {}
unsafe impl Sync for ExecutorLockCell {}

impl ExecutorLockCell {
  /// Creates a new lock cell from read/write closure adapters.
  #[must_use]
  pub fn new<R, W>(read: R, write: W) -> Self
  where
    R: Fn(&mut dyn FnMut(&Box<dyn Executor>)) + Send + Sync + 'static,
    W: Fn(&mut dyn FnMut(&mut Box<dyn Executor>)) + Send + Sync + 'static, {
    let concrete = ArcShared::new(ClosureExecutorLockCellState { read, write });
    let state = (&*concrete as *const ClosureExecutorLockCellState<R, W>).cast::<()>();
    let keeper: ArcShared<dyn Any + Send + Sync> = concrete;
    Self { keeper, state, read: closure_executor_read::<R, W>, write: closure_executor_write::<R, W> }
  }

  /// Executes `f` under the read path of the underlying lock.
  pub fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Executor>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.read)(self.state, &mut |executor| {
        if let Some(callback) = f.take() {
          result.write(callback(executor));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "executor read callback did not run");
    unsafe { result.assume_init() }
  }

  /// Executes `f` under the write path of the underlying lock.
  pub fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Executor>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.write)(self.state, &mut |executor| {
        if let Some(callback) = f.take() {
          result.write(callback(executor));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "executor write callback did not run");
    unsafe { result.assume_init() }
  }
}

impl Clone for ExecutorLockCell {
  fn clone(&self) -> Self {
    Self { keeper: self.keeper.clone(), state: self.state, read: self.read, write: self.write }
  }
}
