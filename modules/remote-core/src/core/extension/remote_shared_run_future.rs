//! Concrete future returned by [`crate::core::extension::RemoteShared::run`].
//!
//! The future checks termination before polling the receiver, so a shared
//! remote that is already shut down completes without waiting for another
//! event.

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use crate::core::extension::{RemoteEventReceiver, RemoteShared, RemotingError};

/// Event-loop future for a shared [`RemoteShared`] handle.
pub struct RemoteSharedRunFuture<'a, S: RemoteEventReceiver + ?Sized> {
  remote:   &'a RemoteShared,
  receiver: &'a mut S,
}

impl<'a, S: RemoteEventReceiver + ?Sized> RemoteSharedRunFuture<'a, S> {
  pub(crate) const fn new(remote: &'a RemoteShared, receiver: &'a mut S) -> Self {
    Self { remote, receiver }
  }
}

impl<S: RemoteEventReceiver + ?Sized> Unpin for RemoteSharedRunFuture<'_, S> {}

impl<S: RemoteEventReceiver + ?Sized> Future for RemoteSharedRunFuture<'_, S> {
  type Output = Result<(), RemotingError>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    // This event-loop future intentionally drains all immediately available
    // events in one poll. If fairness becomes a concern, cap the number of
    // `poll_recv` / `handle_remote_event` iterations per poll.
    loop {
      if this.remote.with_read(|remote| remote.is_terminated()) {
        return Poll::Ready(Ok(()));
      }
      match this.receiver.poll_recv(cx) {
        | Poll::Ready(Some(event)) => {
          match this.remote.with_write(|remote| {
            if remote.is_terminated() {
              return Ok(true);
            }
            remote.handle_remote_event(event)?;
            Ok(remote.is_terminated())
          }) {
            | Ok(true) => return Poll::Ready(Ok(())),
            | Ok(false) => {},
            | Err(error) => return Poll::Ready(Err(error)),
          }
        },
        | Poll::Ready(None) => return Poll::Ready(Err(RemotingError::EventReceiverClosed)),
        | Poll::Pending => return Poll::Pending,
      }
    }
  }
}
