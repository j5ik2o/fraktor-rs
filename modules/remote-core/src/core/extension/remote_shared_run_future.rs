//! Concrete future returned by [`crate::core::extension::RemoteShared::run`].

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
    loop {
      match this.receiver.poll_recv(cx) {
        | Poll::Ready(Some(event)) => {
          if let Err(error) = this.remote.with_write(|remote| remote.handle_remote_event(event)) {
            return Poll::Ready(Err(error));
          }
          if this.remote.with_read(|remote| remote.is_terminated()) {
            return Poll::Ready(Ok(()));
          }
        },
        | Poll::Ready(None) => return Poll::Ready(Err(RemotingError::EventReceiverClosed)),
        | Poll::Pending => return Poll::Pending,
      }
    }
  }
}
