//! Mailbox lock bundle.

use core::{any::Any, mem::MaybeUninit};

use fraktor_utils_core_rs::core::sync::{ArcShared, WeakShared};

use super::{ActorRuntimeLockProvider, BuiltinSpinRuntimeLockProvider};
use crate::core::kernel::{
  actor::{ActorCell, messaging::message_invoker::MessageInvokerShared},
  dispatch::mailbox::MailboxInstrumentation,
};

type UserQueueLockFn = unsafe fn(*const (), &mut dyn FnMut());
type InstrumentationReadFn = unsafe fn(*const (), &mut dyn FnMut(&Option<MailboxInstrumentation>));
type InstrumentationWriteFn = unsafe fn(*const (), &mut dyn FnMut(&mut Option<MailboxInstrumentation>));
type InvokerReadFn = unsafe fn(*const (), &mut dyn FnMut(&Option<MessageInvokerShared>));
type InvokerWriteFn = unsafe fn(*const (), &mut dyn FnMut(&mut Option<MessageInvokerShared>));
type ActorReadFn = unsafe fn(*const (), &mut dyn FnMut(&Option<WeakShared<ActorCell>>));
type ActorWriteFn = unsafe fn(*const (), &mut dyn FnMut(&mut Option<WeakShared<ActorCell>>));

struct ClosureMailboxLockSetState<U, IR, IW, VR, VW, AR, AW>
where
  U: Fn(&mut dyn FnMut()) + Send + Sync + 'static,
  IR: Fn(&mut dyn FnMut(&Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  IW: Fn(&mut dyn FnMut(&mut Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  VR: Fn(&mut dyn FnMut(&Option<MessageInvokerShared>)) + Send + Sync + 'static,
  VW: Fn(&mut dyn FnMut(&mut Option<MessageInvokerShared>)) + Send + Sync + 'static,
  AR: Fn(&mut dyn FnMut(&Option<WeakShared<ActorCell>>)) + Send + Sync + 'static,
  AW: Fn(&mut dyn FnMut(&mut Option<WeakShared<ActorCell>>)) + Send + Sync + 'static, {
  user_queue_lock:       U,
  instrumentation_read:  IR,
  instrumentation_write: IW,
  invoker_read:          VR,
  invoker_write:         VW,
  actor_read:            AR,
  actor_write:           AW,
}

unsafe fn closure_user_queue_lock<U, IR, IW, VR, VW, AR, AW>(state: *const (), f: &mut dyn FnMut())
where
  U: Fn(&mut dyn FnMut()) + Send + Sync + 'static,
  IR: Fn(&mut dyn FnMut(&Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  IW: Fn(&mut dyn FnMut(&mut Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  VR: Fn(&mut dyn FnMut(&Option<MessageInvokerShared>)) + Send + Sync + 'static,
  VW: Fn(&mut dyn FnMut(&mut Option<MessageInvokerShared>)) + Send + Sync + 'static,
  AR: Fn(&mut dyn FnMut(&Option<WeakShared<ActorCell>>)) + Send + Sync + 'static,
  AW: Fn(&mut dyn FnMut(&mut Option<WeakShared<ActorCell>>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureMailboxLockSetState<U, IR, IW, VR, VW, AR, AW>>()) };
  (state.user_queue_lock)(f);
}

unsafe fn closure_instrumentation_read<U, IR, IW, VR, VW, AR, AW>(
  state: *const (),
  f: &mut dyn FnMut(&Option<MailboxInstrumentation>),
) where
  U: Fn(&mut dyn FnMut()) + Send + Sync + 'static,
  IR: Fn(&mut dyn FnMut(&Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  IW: Fn(&mut dyn FnMut(&mut Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  VR: Fn(&mut dyn FnMut(&Option<MessageInvokerShared>)) + Send + Sync + 'static,
  VW: Fn(&mut dyn FnMut(&mut Option<MessageInvokerShared>)) + Send + Sync + 'static,
  AR: Fn(&mut dyn FnMut(&Option<WeakShared<ActorCell>>)) + Send + Sync + 'static,
  AW: Fn(&mut dyn FnMut(&mut Option<WeakShared<ActorCell>>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureMailboxLockSetState<U, IR, IW, VR, VW, AR, AW>>()) };
  (state.instrumentation_read)(f);
}

unsafe fn closure_instrumentation_write<U, IR, IW, VR, VW, AR, AW>(
  state: *const (),
  f: &mut dyn FnMut(&mut Option<MailboxInstrumentation>),
) where
  U: Fn(&mut dyn FnMut()) + Send + Sync + 'static,
  IR: Fn(&mut dyn FnMut(&Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  IW: Fn(&mut dyn FnMut(&mut Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  VR: Fn(&mut dyn FnMut(&Option<MessageInvokerShared>)) + Send + Sync + 'static,
  VW: Fn(&mut dyn FnMut(&mut Option<MessageInvokerShared>)) + Send + Sync + 'static,
  AR: Fn(&mut dyn FnMut(&Option<WeakShared<ActorCell>>)) + Send + Sync + 'static,
  AW: Fn(&mut dyn FnMut(&mut Option<WeakShared<ActorCell>>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureMailboxLockSetState<U, IR, IW, VR, VW, AR, AW>>()) };
  (state.instrumentation_write)(f);
}

unsafe fn closure_invoker_read<U, IR, IW, VR, VW, AR, AW>(
  state: *const (),
  f: &mut dyn FnMut(&Option<MessageInvokerShared>),
) where
  U: Fn(&mut dyn FnMut()) + Send + Sync + 'static,
  IR: Fn(&mut dyn FnMut(&Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  IW: Fn(&mut dyn FnMut(&mut Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  VR: Fn(&mut dyn FnMut(&Option<MessageInvokerShared>)) + Send + Sync + 'static,
  VW: Fn(&mut dyn FnMut(&mut Option<MessageInvokerShared>)) + Send + Sync + 'static,
  AR: Fn(&mut dyn FnMut(&Option<WeakShared<ActorCell>>)) + Send + Sync + 'static,
  AW: Fn(&mut dyn FnMut(&mut Option<WeakShared<ActorCell>>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureMailboxLockSetState<U, IR, IW, VR, VW, AR, AW>>()) };
  (state.invoker_read)(f);
}

unsafe fn closure_invoker_write<U, IR, IW, VR, VW, AR, AW>(
  state: *const (),
  f: &mut dyn FnMut(&mut Option<MessageInvokerShared>),
) where
  U: Fn(&mut dyn FnMut()) + Send + Sync + 'static,
  IR: Fn(&mut dyn FnMut(&Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  IW: Fn(&mut dyn FnMut(&mut Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  VR: Fn(&mut dyn FnMut(&Option<MessageInvokerShared>)) + Send + Sync + 'static,
  VW: Fn(&mut dyn FnMut(&mut Option<MessageInvokerShared>)) + Send + Sync + 'static,
  AR: Fn(&mut dyn FnMut(&Option<WeakShared<ActorCell>>)) + Send + Sync + 'static,
  AW: Fn(&mut dyn FnMut(&mut Option<WeakShared<ActorCell>>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureMailboxLockSetState<U, IR, IW, VR, VW, AR, AW>>()) };
  (state.invoker_write)(f);
}

unsafe fn closure_actor_read<U, IR, IW, VR, VW, AR, AW>(
  state: *const (),
  f: &mut dyn FnMut(&Option<WeakShared<ActorCell>>),
) where
  U: Fn(&mut dyn FnMut()) + Send + Sync + 'static,
  IR: Fn(&mut dyn FnMut(&Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  IW: Fn(&mut dyn FnMut(&mut Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  VR: Fn(&mut dyn FnMut(&Option<MessageInvokerShared>)) + Send + Sync + 'static,
  VW: Fn(&mut dyn FnMut(&mut Option<MessageInvokerShared>)) + Send + Sync + 'static,
  AR: Fn(&mut dyn FnMut(&Option<WeakShared<ActorCell>>)) + Send + Sync + 'static,
  AW: Fn(&mut dyn FnMut(&mut Option<WeakShared<ActorCell>>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureMailboxLockSetState<U, IR, IW, VR, VW, AR, AW>>()) };
  (state.actor_read)(f);
}

unsafe fn closure_actor_write<U, IR, IW, VR, VW, AR, AW>(
  state: *const (),
  f: &mut dyn FnMut(&mut Option<WeakShared<ActorCell>>),
) where
  U: Fn(&mut dyn FnMut()) + Send + Sync + 'static,
  IR: Fn(&mut dyn FnMut(&Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  IW: Fn(&mut dyn FnMut(&mut Option<MailboxInstrumentation>)) + Send + Sync + 'static,
  VR: Fn(&mut dyn FnMut(&Option<MessageInvokerShared>)) + Send + Sync + 'static,
  VW: Fn(&mut dyn FnMut(&mut Option<MessageInvokerShared>)) + Send + Sync + 'static,
  AR: Fn(&mut dyn FnMut(&Option<WeakShared<ActorCell>>)) + Send + Sync + 'static,
  AW: Fn(&mut dyn FnMut(&mut Option<WeakShared<ActorCell>>)) + Send + Sync + 'static, {
  let state = unsafe { &*(state.cast::<ClosureMailboxLockSetState<U, IR, IW, VR, VW, AR, AW>>()) };
  (state.actor_write)(f);
}

/// Mailbox-internal lock bundle created by a runtime lock provider.
pub struct MailboxLockSet {
  keeper:                ArcShared<dyn Any + Send + Sync>,
  state:                 *const (),
  user_queue_lock:       UserQueueLockFn,
  instrumentation_read:  InstrumentationReadFn,
  instrumentation_write: InstrumentationWriteFn,
  invoker_read:          InvokerReadFn,
  invoker_write:         InvokerWriteFn,
  actor_read:            ActorReadFn,
  actor_write:           ActorWriteFn,
}

unsafe impl Send for MailboxLockSet {}
unsafe impl Sync for MailboxLockSet {}

impl MailboxLockSet {
  /// Creates a mailbox lock bundle backed by the builtin spin provider.
  #[must_use]
  pub fn builtin_spin() -> Self {
    BuiltinSpinRuntimeLockProvider.new_mailbox_lock_set()
  }

  /// Creates a new mailbox lock bundle from closure adapters.
  #[must_use]
  pub fn new<U, IR, IW, VR, VW, AR, AW>(
    user_queue_lock: U,
    instrumentation_read: IR,
    instrumentation_write: IW,
    invoker_read: VR,
    invoker_write: VW,
    actor_read: AR,
    actor_write: AW,
  ) -> Self
  where
    U: Fn(&mut dyn FnMut()) + Send + Sync + 'static,
    IR: Fn(&mut dyn FnMut(&Option<MailboxInstrumentation>)) + Send + Sync + 'static,
    IW: Fn(&mut dyn FnMut(&mut Option<MailboxInstrumentation>)) + Send + Sync + 'static,
    VR: Fn(&mut dyn FnMut(&Option<MessageInvokerShared>)) + Send + Sync + 'static,
    VW: Fn(&mut dyn FnMut(&mut Option<MessageInvokerShared>)) + Send + Sync + 'static,
    AR: Fn(&mut dyn FnMut(&Option<WeakShared<ActorCell>>)) + Send + Sync + 'static,
    AW: Fn(&mut dyn FnMut(&mut Option<WeakShared<ActorCell>>)) + Send + Sync + 'static, {
    let concrete = ArcShared::new(ClosureMailboxLockSetState {
      user_queue_lock,
      instrumentation_read,
      instrumentation_write,
      invoker_read,
      invoker_write,
      actor_read,
      actor_write,
    });
    let state = (&*concrete as *const ClosureMailboxLockSetState<U, IR, IW, VR, VW, AR, AW>).cast::<()>();
    let keeper: ArcShared<dyn Any + Send + Sync> = concrete;
    Self {
      keeper,
      state,
      user_queue_lock: closure_user_queue_lock::<U, IR, IW, VR, VW, AR, AW>,
      instrumentation_read: closure_instrumentation_read::<U, IR, IW, VR, VW, AR, AW>,
      instrumentation_write: closure_instrumentation_write::<U, IR, IW, VR, VW, AR, AW>,
      invoker_read: closure_invoker_read::<U, IR, IW, VR, VW, AR, AW>,
      invoker_write: closure_invoker_write::<U, IR, IW, VR, VW, AR, AW>,
      actor_read: closure_actor_read::<U, IR, IW, VR, VW, AR, AW>,
      actor_write: closure_actor_write::<U, IR, IW, VR, VW, AR, AW>,
    }
  }

  /// Executes `f` while holding the mailbox user-queue serialization lock.
  pub fn with_user_queue_lock<R>(&self, f: impl FnOnce() -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.user_queue_lock)(self.state, &mut || {
        if let Some(callback) = f.take() {
          result.write(callback());
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "mailbox user queue callback did not run");
    unsafe { result.assume_init() }
  }

  /// Executes `f` against the instrumentation slot under the provider lock.
  pub fn with_instrumentation_read<R>(&self, f: impl FnOnce(&Option<MailboxInstrumentation>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.instrumentation_read)(self.state, &mut |instrumentation| {
        if let Some(callback) = f.take() {
          result.write(callback(instrumentation));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "mailbox instrumentation read callback did not run");
    unsafe { result.assume_init() }
  }

  /// Executes `f` mutably against the instrumentation slot under the provider lock.
  pub fn with_instrumentation_write<R>(&self, f: impl FnOnce(&mut Option<MailboxInstrumentation>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.instrumentation_write)(self.state, &mut |instrumentation| {
        if let Some(callback) = f.take() {
          result.write(callback(instrumentation));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "mailbox instrumentation write callback did not run");
    unsafe { result.assume_init() }
  }

  /// Executes `f` against the invoker slot under the provider lock.
  pub fn with_invoker_read<R>(&self, f: impl FnOnce(&Option<MessageInvokerShared>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.invoker_read)(self.state, &mut |invoker| {
        if let Some(callback) = f.take() {
          result.write(callback(invoker));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "mailbox invoker read callback did not run");
    unsafe { result.assume_init() }
  }

  /// Executes `f` mutably against the invoker slot under the provider lock.
  pub fn with_invoker_write<R>(&self, f: impl FnOnce(&mut Option<MessageInvokerShared>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.invoker_write)(self.state, &mut |invoker| {
        if let Some(callback) = f.take() {
          result.write(callback(invoker));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "mailbox invoker write callback did not run");
    unsafe { result.assume_init() }
  }

  /// Executes `f` against the actor slot under the provider lock.
  pub fn with_actor_read<R>(&self, f: impl FnOnce(&Option<WeakShared<ActorCell>>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.actor_read)(self.state, &mut |actor| {
        if let Some(callback) = f.take() {
          result.write(callback(actor));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "mailbox actor read callback did not run");
    unsafe { result.assume_init() }
  }

  /// Executes `f` mutably against the actor slot under the provider lock.
  pub fn with_actor_write<R>(&self, f: impl FnOnce(&mut Option<WeakShared<ActorCell>>) -> R) -> R {
    let _keep_alive = &self.keeper;
    let mut f = Some(f);
    let mut result = MaybeUninit::<R>::uninit();
    let mut has_result = false;
    unsafe {
      (self.actor_write)(self.state, &mut |actor| {
        if let Some(callback) = f.take() {
          result.write(callback(actor));
          has_result = true;
        }
      });
    }
    debug_assert!(has_result, "mailbox actor write callback did not run");
    unsafe { result.assume_init() }
  }
}

impl Clone for MailboxLockSet {
  fn clone(&self) -> Self {
    Self {
      keeper:                self.keeper.clone(),
      state:                 self.state,
      user_queue_lock:       self.user_queue_lock,
      instrumentation_read:  self.instrumentation_read,
      instrumentation_write: self.instrumentation_write,
      invoker_read:          self.invoker_read,
      invoker_write:         self.invoker_write,
      actor_read:            self.actor_read,
      actor_write:           self.actor_write,
    }
  }
}
