//! Concrete future returned by [`crate::core::extension::Remote::run`].

use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use crate::core::extension::{Remote, RemoteEventReceiver, RemotingError};

/// Event-loop future for an exclusively owned [`Remote`].
pub struct RemoteRunFuture<'a, S: RemoteEventReceiver + ?Sized> {
  remote:   &'a mut Remote,
  receiver: &'a mut S,
}

impl<'a, S: RemoteEventReceiver + ?Sized> RemoteRunFuture<'a, S> {
  pub(crate) const fn new(remote: &'a mut Remote, receiver: &'a mut S) -> Self {
    Self { remote, receiver }
  }
}

impl<S: RemoteEventReceiver + ?Sized> Unpin for RemoteRunFuture<'_, S> {}

impl<S: RemoteEventReceiver + ?Sized> Future for RemoteRunFuture<'_, S> {
  type Output = Result<(), RemotingError>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = self.get_mut();
    loop {
      if this.remote.should_stop_event_loop() {
        return Poll::Ready(Ok(()));
      }
      match this.receiver.poll_recv(cx) {
        | Poll::Ready(Some(event)) => {
          if let Err(error) = this.remote.handle_remote_event(event) {
            return Poll::Ready(Err(error));
          }
          if this.remote.should_stop_event_loop() {
            return Poll::Ready(Ok(()));
          }
        },
        | Poll::Ready(None) => return Poll::Ready(Err(RemotingError::EventReceiverClosed)),
        | Poll::Pending => return Poll::Pending,
      }
    }
  }
}
