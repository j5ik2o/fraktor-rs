//! Mailbox-local lock-free MPSC queue primitive.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
#[cfg(not(loom))]
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use core::{marker::PhantomData, ptr};

#[cfg(loom)]
use loom::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

struct Node<T> {
  message: T,
  next:    *mut Node<T>,
}

impl<T> Node<T> {
  fn new(message: T) -> *mut Self {
    Box::into_raw(Box::new(Self { message, next: ptr::null_mut() }))
  }
}

/// Lock-free MPSC queue used by the standard unbounded mailbox user queue.
pub(crate) struct LockFreeMpscQueue<T> {
  head:            AtomicPtr<Node<T>>,
  pending:         AtomicPtr<Node<T>>,
  len:             AtomicUsize,
  in_flight:       AtomicUsize,
  closed:          AtomicBool,
  consumer_active: AtomicBool,
  _marker:         PhantomData<T>,
}

// SAFETY: the queue transfers owned `T` values between producer threads and the
// consumer without exposing shared references to `T`; requiring `T: Send` is
// sufficient for both moving the queue and sharing queue references.
unsafe impl<T: Send> Send for LockFreeMpscQueue<T> {}
unsafe impl<T: Send> Sync for LockFreeMpscQueue<T> {}

impl<T> Default for LockFreeMpscQueue<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> LockFreeMpscQueue<T> {
  /// Creates an empty queue.
  #[must_use]
  #[cfg(not(loom))]
  pub(crate) const fn new() -> Self {
    Self {
      head:            AtomicPtr::new(ptr::null_mut()),
      pending:         AtomicPtr::new(ptr::null_mut()),
      len:             AtomicUsize::new(0),
      in_flight:       AtomicUsize::new(0),
      closed:          AtomicBool::new(false),
      consumer_active: AtomicBool::new(false),
      _marker:         PhantomData,
    }
  }

  /// Creates an empty queue.
  #[must_use]
  #[cfg(loom)]
  pub(crate) fn new() -> Self {
    Self {
      head:            AtomicPtr::new(ptr::null_mut()),
      pending:         AtomicPtr::new(ptr::null_mut()),
      len:             AtomicUsize::new(0),
      in_flight:       AtomicUsize::new(0),
      closed:          AtomicBool::new(false),
      consumer_active: AtomicBool::new(false),
      _marker:         PhantomData,
    }
  }

  /// Pushes an item unless the queue-local close protocol has closed the queue.
  pub(crate) fn push(&self, message: T) -> Result<(), T> {
    let Some(_guard) = self.enter_producer() else {
      return Err(message);
    };

    let node = Node::new(message);
    self.len.fetch_add(1, Ordering::Release);
    loop {
      let current_head = self.head.load(Ordering::Acquire);
      // SAFETY: `node` is uniquely owned by this producer until the compare-exchange succeeds.
      // Updating its next pointer does not touch any node reachable from the queue.
      unsafe {
        (*node).next = current_head;
      }
      if self.head.compare_exchange(current_head, node, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        return Ok(());
      }
    }
  }

  /// Pops one item in FIFO order.
  #[must_use]
  pub(crate) fn pop(&self) -> Option<T> {
    let _guard = self.enter_consumer();
    self.pop_with_consumer_held()
  }

  /// Publishes close and waits for every producer that entered the protocol.
  pub(crate) fn close(&self) {
    self.closed.store(true, Ordering::SeqCst);
    while self.in_flight.load(Ordering::SeqCst) != 0 {
      spin_loop();
    }
  }

  /// Closes the queue and drops all queued items.
  pub(crate) fn close_and_drain(&self) {
    self.close();
    let _guard = self.enter_consumer();
    while self.pop_with_consumer_held().is_some() {}
  }

  /// Returns the current queue length.
  #[must_use]
  pub(crate) fn len(&self) -> usize {
    self.len.load(Ordering::Acquire)
  }

  fn enter_producer(&self) -> Option<ProducerGuard<'_>> {
    self.in_flight.fetch_add(1, Ordering::SeqCst);
    if self.closed.load(Ordering::SeqCst) {
      self.in_flight.fetch_sub(1, Ordering::SeqCst);
      None
    } else {
      Some(ProducerGuard { in_flight: &self.in_flight })
    }
  }

  fn enter_consumer(&self) -> ConsumerGuard<'_> {
    while self.consumer_active.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
      spin_loop();
    }
    ConsumerGuard { consumer_active: &self.consumer_active }
  }

  fn pop_with_consumer_held(&self) -> Option<T> {
    let pending = self.pending.load(Ordering::Acquire);
    if !pending.is_null() {
      return Some(self.pop_pending_node(pending));
    }

    let head = self.head.swap(ptr::null_mut(), Ordering::AcqRel);
    if head.is_null() {
      return None;
    }

    self.pending.store(Self::reverse_list(head), Ordering::Release);
    let pending = self.pending.load(Ordering::Acquire);
    if pending.is_null() { None } else { Some(self.pop_pending_node(pending)) }
  }

  fn pop_pending_node(&self, node_ptr: *mut Node<T>) -> T {
    // SAFETY: the consumer guard gives this method exclusive access to `pending`.
    // `node_ptr` was read from `pending`, which only contains nodes allocated by `Node::new`.
    let node = unsafe { Box::from_raw(node_ptr) };
    let Node { message, next } = *node;
    self.pending.store(next, Ordering::Release);
    self.len.fetch_sub(1, Ordering::Release);
    message
  }

  fn reverse_list(mut head: *mut Node<T>) -> *mut Node<T> {
    let mut prev = ptr::null_mut();
    while !head.is_null() {
      // SAFETY: `head` is part of the chain just removed from `self.head`.
      // The consumer owns this detached chain while reversing it.
      let next = unsafe { (*head).next };
      // SAFETY: the detached chain is exclusively owned by this consumer.
      unsafe {
        (*head).next = prev;
      }
      prev = head;
      head = next;
    }
    prev
  }

  unsafe fn free_chain(mut head: *mut Node<T>) {
    while !head.is_null() {
      // SAFETY: `head` is a valid node in a chain allocated by `Node::new`.
      let next = unsafe { (*head).next };
      // SAFETY: each node in the chain is visited once, so ownership is restored once.
      drop(unsafe { Box::from_raw(head) });
      head = next;
    }
  }
}

impl<T> Drop for LockFreeMpscQueue<T> {
  fn drop(&mut self) {
    let head = self.head.swap(ptr::null_mut(), Ordering::Relaxed);
    let pending = self.pending.swap(ptr::null_mut(), Ordering::Relaxed);
    // SAFETY: `drop` has exclusive access to the queue, so no producer or consumer can
    // concurrently mutate the detached chains.
    unsafe {
      Self::free_chain(head);
      Self::free_chain(pending);
    }
  }
}

#[cfg(loom)]
fn spin_loop() {
  loom::sync::atomic::spin_loop_hint();
}

#[cfg(not(loom))]
fn spin_loop() {
  core::hint::spin_loop();
}

struct ProducerGuard<'a> {
  in_flight: &'a AtomicUsize,
}

impl Drop for ProducerGuard<'_> {
  fn drop(&mut self) {
    self.in_flight.fetch_sub(1, Ordering::SeqCst);
  }
}

struct ConsumerGuard<'a> {
  consumer_active: &'a AtomicBool,
}

impl Drop for ConsumerGuard<'_> {
  fn drop(&mut self) {
    self.consumer_active.store(false, Ordering::Release);
  }
}
