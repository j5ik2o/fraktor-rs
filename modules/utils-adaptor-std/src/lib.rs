//! Std adapter helpers for actor-runtime lock providers.

use std::{
  collections::{BTreeSet, HashMap},
  sync::{Arc, Mutex, MutexGuard, OnceLock},
  thread::ThreadId,
};

use fraktor_actor_core_rs::core::kernel::{
  actor::{ActorCell, actor_ref::ActorRefSender, messaging::message_invoker::MessageInvokerShared},
  dispatch::{
    dispatcher::{Executor, MessageDispatcher},
    mailbox::MailboxInstrumentation,
  },
  runtime_lock_provider::{
    ActorRuntimeLockProvider, DispatcherLockCell, ExecutorLockCell, MailboxLockSet, SenderLockCell,
  },
};
use fraktor_utils_core_rs::core::sync::{ArcShared, WeakShared};
use portable_atomic::AtomicUsize;

static NEXT_DEBUG_LOCK_ID: AtomicUsize = AtomicUsize::new(1);
static HELD_DEBUG_LOCKS: OnceLock<Mutex<HashMap<ThreadId, BTreeSet<usize>>>> = OnceLock::new();

/// Returns a runtime lock provider backed by `std::sync::Mutex`.
#[must_use]
pub fn new_std_runtime_lock_provider() -> ArcShared<dyn ActorRuntimeLockProvider> {
  let provider: ArcShared<dyn ActorRuntimeLockProvider> = ArcShared::new(StdRuntimeLockProvider);
  provider
}

/// Returns a runtime lock provider that panics on same-thread reentrant lock acquisition.
#[must_use]
pub fn new_debug_runtime_lock_provider() -> ArcShared<dyn ActorRuntimeLockProvider> {
  let provider: ArcShared<dyn ActorRuntimeLockProvider> = ArcShared::new(DebugRuntimeLockProvider);
  provider
}

struct StdRuntimeLockProvider;

impl ActorRuntimeLockProvider for StdRuntimeLockProvider {
  fn new_dispatcher_cell(&self, dispatcher: Box<dyn MessageDispatcher>) -> DispatcherLockCell {
    let read = Arc::new(Mutex::new(dispatcher));
    let write = Arc::clone(&read);
    DispatcherLockCell::new(
      move |f| {
        let guard = lock_unpoison(&read);
        f(&guard);
      },
      move |f| {
        let mut guard = lock_unpoison(&write);
        f(&mut guard);
      },
    )
  }

  fn new_executor_cell(&self, executor: Box<dyn Executor>) -> ExecutorLockCell {
    let read = Arc::new(Mutex::new(executor));
    let write = Arc::clone(&read);
    ExecutorLockCell::new(
      move |f| {
        let guard = lock_unpoison(&read);
        f(&guard);
      },
      move |f| {
        let mut guard = lock_unpoison(&write);
        f(&mut guard);
      },
    )
  }

  fn new_sender_cell(&self, sender: Box<dyn ActorRefSender>) -> SenderLockCell {
    let read = Arc::new(Mutex::new(sender));
    let write = Arc::clone(&read);
    SenderLockCell::new(
      move |f| {
        let guard = lock_unpoison(&read);
        f(&guard);
      },
      move |f| {
        let mut guard = lock_unpoison(&write);
        f(&mut guard);
      },
    )
  }

  fn new_mailbox_lock_set(&self) -> MailboxLockSet {
    let user_queue_lock = Arc::new(Mutex::new(()));
    let instrumentation = Arc::new(Mutex::new(None::<MailboxInstrumentation>));
    let invoker = Arc::new(Mutex::new(None::<MessageInvokerShared>));
    let actor = Arc::new(Mutex::new(None::<WeakShared<ActorCell>>));

    let user_queue_write = Arc::clone(&user_queue_lock);
    let instrumentation_read = Arc::clone(&instrumentation);
    let instrumentation_write = Arc::clone(&instrumentation);
    let invoker_read = Arc::clone(&invoker);
    let invoker_write = Arc::clone(&invoker);
    let actor_read = Arc::clone(&actor);
    let actor_write = Arc::clone(&actor);

    MailboxLockSet::new(
      move |f| {
        let _guard = lock_unpoison(&user_queue_write);
        f();
      },
      move |f| {
        let guard = lock_unpoison(&instrumentation_read);
        f(&guard);
      },
      move |f| {
        let mut guard = lock_unpoison(&instrumentation_write);
        f(&mut guard);
      },
      move |f| {
        let guard = lock_unpoison(&invoker_read);
        f(&guard);
      },
      move |f| {
        let mut guard = lock_unpoison(&invoker_write);
        f(&mut guard);
      },
      move |f| {
        let guard = lock_unpoison(&actor_read);
        f(&guard);
      },
      move |f| {
        let mut guard = lock_unpoison(&actor_write);
        f(&mut guard);
      },
    )
  }
}

struct DebugRuntimeLockProvider;

impl ActorRuntimeLockProvider for DebugRuntimeLockProvider {
  fn new_dispatcher_cell(&self, dispatcher: Box<dyn MessageDispatcher>) -> DispatcherLockCell {
    let read = Arc::new(DebugLock::new(dispatcher));
    let write = Arc::clone(&read);
    DispatcherLockCell::new(
      move |f| {
        let guard = read.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = write.lock();
        f(&mut guard);
      },
    )
  }

  fn new_executor_cell(&self, executor: Box<dyn Executor>) -> ExecutorLockCell {
    let read = Arc::new(DebugLock::new(executor));
    let write = Arc::clone(&read);
    ExecutorLockCell::new(
      move |f| {
        let guard = read.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = write.lock();
        f(&mut guard);
      },
    )
  }

  fn new_sender_cell(&self, sender: Box<dyn ActorRefSender>) -> SenderLockCell {
    let read = Arc::new(DebugLock::new(sender));
    let write = Arc::clone(&read);
    SenderLockCell::new(
      move |f| {
        let guard = read.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = write.lock();
        f(&mut guard);
      },
    )
  }

  fn new_mailbox_lock_set(&self) -> MailboxLockSet {
    let user_queue_lock = Arc::new(DebugLock::new(()));
    let instrumentation = Arc::new(DebugLock::new(None::<MailboxInstrumentation>));
    let invoker = Arc::new(DebugLock::new(None::<MessageInvokerShared>));
    let actor = Arc::new(DebugLock::new(None::<WeakShared<ActorCell>>));

    let user_queue_write = Arc::clone(&user_queue_lock);
    let instrumentation_read = Arc::clone(&instrumentation);
    let instrumentation_write = Arc::clone(&instrumentation);
    let invoker_read = Arc::clone(&invoker);
    let invoker_write = Arc::clone(&invoker);
    let actor_read = Arc::clone(&actor);
    let actor_write = Arc::clone(&actor);

    MailboxLockSet::new(
      move |f| {
        let _guard = user_queue_write.lock();
        f();
      },
      move |f| {
        let guard = instrumentation_read.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = instrumentation_write.lock();
        f(&mut guard);
      },
      move |f| {
        let guard = invoker_read.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = invoker_write.lock();
        f(&mut guard);
      },
      move |f| {
        let guard = actor_read.lock();
        f(&guard);
      },
      move |f| {
        let mut guard = actor_write.lock();
        f(&mut guard);
      },
    )
  }
}

fn lock_unpoison<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
  match mutex.lock() {
    | Ok(guard) => guard,
    | Err(poisoned) => poisoned.into_inner(),
  }
}

fn held_debug_locks() -> &'static Mutex<HashMap<ThreadId, BTreeSet<usize>>> {
  HELD_DEBUG_LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct DebugLock<T> {
  id:    usize,
  inner: Mutex<T>,
}

impl<T> DebugLock<T> {
  fn new(value: T) -> Self {
    Self { id: NEXT_DEBUG_LOCK_ID.fetch_add(1, portable_atomic::Ordering::Relaxed), inner: Mutex::new(value) }
  }

  fn lock(&self) -> DebugLockGuard<'_, T> {
    let thread_id = std::thread::current().id();
    {
      let mut held = lock_unpoison(held_debug_locks());
      let entry = held.entry(thread_id).or_default();
      assert!(entry.insert(self.id), "same-thread runtime lock re-entry detected for debug runtime lock {}", self.id,);
    }

    let guard = lock_unpoison(&self.inner);
    DebugLockGuard { id: self.id, thread_id, guard }
  }
}

struct DebugLockGuard<'a, T> {
  id:        usize,
  thread_id: ThreadId,
  guard:     MutexGuard<'a, T>,
}

impl<T> Drop for DebugLockGuard<'_, T> {
  fn drop(&mut self) {
    let mut held = lock_unpoison(held_debug_locks());
    if let Some(entry) = held.get_mut(&self.thread_id) {
      entry.remove(&self.id);
      if entry.is_empty() {
        held.remove(&self.thread_id);
      }
    }
  }
}

impl<T> core::ops::Deref for DebugLockGuard<'_, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<T> core::ops::DerefMut for DebugLockGuard<'_, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}
