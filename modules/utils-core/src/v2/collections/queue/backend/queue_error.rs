use crate::{collections::queue::QueueError, v2::sync::SharedError};

impl<T> From<SharedError> for QueueError<T> {
  fn from(err: SharedError) -> Self {
    match err {
      | SharedError::Poisoned => QueueError::Disconnected,
      | SharedError::BorrowConflict | SharedError::InterruptContext => QueueError::WouldBlock,
    }
  }
}
