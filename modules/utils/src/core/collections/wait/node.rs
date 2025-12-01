use core::task::{Context, Poll, Waker};

const STATE_PENDING: u8 = 0;
const STATE_COMPLETED: u8 = 1;
const STATE_CANCELLED: u8 = 2;

/// Internal node representing a single waiter.
#[derive(Debug)]
pub struct WaitNode<E> {
  state:  u8,
  waker:  Option<Waker>,
  result: Option<Result<(), E>>,
}

impl<E> WaitNode<E> {
  /// Creates a new pending waiter node.
  pub const fn new() -> Self {
    Self { state: STATE_PENDING, waker: None, result: None }
  }

  /// Completes the waiter with the provided result.
  pub fn complete(&mut self, value: Result<(), E>) -> bool {
    if self.state != STATE_PENDING {
      return false;
    }

    self.state = STATE_COMPLETED;
    self.result = Some(value);

    if let Some(waker) = self.waker.take() {
      waker.wake();
    }

    true
  }

  /// Marks the waiter as cancelled.
  pub fn cancel(&mut self) {
    if self.state == STATE_PENDING {
      self.state = STATE_CANCELLED;
      self.waker.take();
    }
  }

  /// Polls the waiter for completion.
  pub fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
    match self.state {
      | STATE_COMPLETED => Poll::Ready(()),
      | STATE_CANCELLED => Poll::Pending,
      | _ => {
        self.waker = Some(cx.waker().clone());

        if self.state == STATE_COMPLETED { Poll::Ready(()) } else { Poll::Pending }
      },
    }
  }

  /// Completes the waiter with the specified error.
  pub fn complete_with_error(&mut self, error: E) {
    let _ = self.complete(Err(error));
  }

  /// Completes the waiter successfully.
  pub fn complete_ok(&mut self) -> bool {
    self.complete(Ok(()))
  }

  /// Takes the completion result if available.
  pub const fn take_result(&mut self) -> Option<Result<(), E>> {
    self.result.take()
  }
}

impl<E> Default for WaitNode<E> {
  fn default() -> Self {
    Self::new()
  }
}
