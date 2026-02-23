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
  ///
  /// Returns `(completed, waker)`. The caller MUST call `waker.wake()` after
  /// releasing any outer lock to avoid deadlock with spinlock-based mutexes.
  pub fn complete(&mut self, value: Result<(), E>) -> (bool, Option<Waker>) {
    if self.state != STATE_PENDING {
      return (false, None);
    }

    self.state = STATE_COMPLETED;
    self.result = Some(value);

    // wakerをtakeして返す。ロック外でwake()すべき
    let waker = self.waker.take();
    (true, waker)
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
  ///
  /// Returns the waker if one was registered. The caller MUST call
  /// `waker.wake()` after releasing any outer lock.
  pub fn complete_with_error(&mut self, error: E) -> Option<Waker> {
    let (_, waker) = self.complete(Err(error));
    waker
  }

  /// Completes the waiter successfully.
  ///
  /// Returns `(completed, waker)`. The caller MUST call `waker.wake()` after
  /// releasing any outer lock.
  pub fn complete_ok(&mut self) -> (bool, Option<Waker>) {
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
