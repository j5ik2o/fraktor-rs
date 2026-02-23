use alloc::boxed::Box;
use core::{
  ptr,
  sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use crate::core::messaging::system_message::SystemMessage;

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
pub(crate) struct SystemQueue {
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
  pub(crate) const fn new() -> Self {
    Self {
      head:    AtomicPtr::new(ptr::null_mut()),
      pending: AtomicPtr::new(ptr::null_mut()),
      len:     AtomicUsize::new(0),
    }
  }

  /// Pushes a new system message onto the queue.
  pub(crate) fn push(&self, message: SystemMessage) {
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
  pub(crate) fn pop(&self) -> Option<SystemMessage> {
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
      // CAS: pendingがまだnullの場合のみ書き込み。競合時はheadに戻す
      if self.pending.compare_exchange(ptr::null_mut(), fifo_head, Ordering::AcqRel, Ordering::Acquire).is_err() {
        self.return_to_head(fifo_head);
      }
    }
  }

  /// Re-pushes a FIFO chain back to the head stack without modifying len.
  ///
  /// # Safety
  ///
  /// The chain must consist of valid nodes originally taken from head.
  fn return_to_head(&self, mut node: *mut Node) {
    while !node.is_null() {
      let next = unsafe { (*node).next };
      loop {
        let current_head = self.head.load(Ordering::Acquire);
        unsafe {
          (*node).next = current_head;
        }
        if self.head.compare_exchange(current_head, node, Ordering::AcqRel, Ordering::Acquire).is_ok() {
          break;
        }
      }
      node = next;
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
  pub(crate) fn len(&self) -> usize {
    self.len.load(Ordering::Acquire)
  }

  /// Returns `true` when the queue has no pending elements.
  pub(crate) fn is_empty(&self) -> bool {
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

  /// Frees all nodes in a linked list starting from `head`.
  ///
  /// # Safety
  ///
  /// `head` must be a valid pointer to a `Node` chain allocated via `Box::into_raw`, or null.
  unsafe fn free_chain(mut head: *mut Node) {
    while !head.is_null() {
      let next = unsafe { (*head).next };
      let _ = unsafe { Box::from_raw(head) };
      head = next;
    }
  }
}

impl Drop for SystemQueue {
  fn drop(&mut self) {
    // SAFETY: drop時には排他アクセスが保証されている（&mut self）
    let head = *self.head.get_mut();
    let pending = *self.pending.get_mut();
    unsafe {
      Self::free_chain(head);
      Self::free_chain(pending);
    }
  }
}
