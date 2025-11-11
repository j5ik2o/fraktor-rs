use alloc::boxed::Box;
use core::{
  ptr,
  sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use crate::messaging::SystemMessage;

#[cfg(test)]
mod tests;

struct Node {
  message: SystemMessage,
  next:    *mut Node,
}

impl Node {
  fn new(message: SystemMessage) -> *mut Node {
    Box::into_raw(Box::new(Self { message, next: ptr::null_mut() }))
  }
}

/// Lock-free system queue that guarantees FIFO processing order by using
/// a Treiber stack for writers and a pending FIFO list for readers.
pub struct SystemQueue {
  head:    AtomicPtr<Node>,
  pending: AtomicPtr<Node>,
  len:     AtomicUsize,
}

impl Default for SystemQueue {
  fn default() -> Self {
    Self::new()
  }
}

impl SystemQueue {
  /// Creates an empty queue.
  #[must_use]
  pub const fn new() -> Self {
    Self {
      head:    AtomicPtr::new(ptr::null_mut()),
      pending: AtomicPtr::new(ptr::null_mut()),
      len:     AtomicUsize::new(0),
    }
  }

  /// Pushes a new system message onto the queue.
  pub fn push(&self, message: SystemMessage) {
    let node = Node::new(message);
    loop {
      let current_head = self.head.load(Ordering::Acquire);
      unsafe {
        (*node).next = current_head;
      }
      if self.head.compare_exchange(current_head, node, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        self.len.fetch_add(1, Ordering::Release);
        return;
      }
    }
  }

  /// Pops the next message in FIFO order.
  pub fn pop(&self) -> Option<SystemMessage> {
    loop {
      let pending = self.pending.load(Ordering::Acquire);
      if !pending.is_null() {
        if let Some(message) = self.pop_from_pending(pending) {
          return Some(message);
        }
        continue;
      }

      let head = self.head.swap(ptr::null_mut(), Ordering::AcqRel);
      if head.is_null() {
        return None;
      }
      let fifo_head = Self::reverse_list(head);
      self.pending.store(fifo_head, Ordering::Release);
    }
  }

  fn pop_from_pending(&self, pending_ptr: *mut Node) -> Option<SystemMessage> {
    let next = unsafe { (*pending_ptr).next };
    if self.pending.compare_exchange(pending_ptr, next, Ordering::AcqRel, Ordering::Acquire).is_ok() {
      self.len.fetch_sub(1, Ordering::Release);
      let node = unsafe { Box::from_raw(pending_ptr) };
      let message = node.message;
      Some(message)
    } else {
      None
    }
  }

  /// Returns the approximate queue length.
  pub fn len(&self) -> usize {
    self.len.load(Ordering::Acquire)
  }

  /// Returns `true` when the queue has no pending elements.
  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  fn reverse_list(mut head: *mut Node) -> *mut Node {
    let mut prev = ptr::null_mut();
    while !head.is_null() {
      let next = unsafe { (*head).next };
      unsafe {
        (*head).next = prev;
      }
      prev = head;
      head = next;
    }
    prev
  }
}
