use crate::{collections::queue_old::QueueError, sync::shared_error::SharedError};

impl<T> From<SharedError> for QueueError<T> {
  fn from(err: SharedError) -> Self {
    match err {
      | SharedError::Poisoned => QueueError::Disconnected,
      | SharedError::BorrowConflict | SharedError::InterruptContext => QueueError::WouldBlock,
    }
  }
}
