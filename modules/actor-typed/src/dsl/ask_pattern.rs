//! Standalone ask helpers mirroring Pekko's `AskPattern`.

#[cfg(test)]
mod tests;

use core::time::Duration;

use fraktor_actor_core_kernel_rs::pattern::{complete_with_timeout, install_ask_timeout};
use fraktor_utils_core_rs::core::sync::SharedAccess;

use crate::{
  TypedActorRef,
  dsl::{StatusReply, TypedAskResponse},
};

/// Standalone ask helpers for typed actor references.
pub struct AskPattern;

impl AskPattern {
  /// Sends a typed request and installs timeout handling on the returned future.
  #[must_use]
  pub fn ask<Req, Res, F>(target: &mut TypedActorRef<Req>, build: F, timeout: Duration) -> TypedAskResponse<Res>
  where
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
    F: FnOnce(TypedActorRef<Res>) -> Req, {
    let response = target.ask(build);
    Self::install_timeout(target, &response, timeout);
    response
  }

  /// Sends a typed status request and installs timeout handling on the returned future.
  #[must_use]
  pub fn ask_with_status<Req, Res, F>(
    target: &mut TypedActorRef<Req>,
    build: F,
    timeout: Duration,
  ) -> TypedAskResponse<StatusReply<Res>>
  where
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static,
    F: FnOnce(TypedActorRef<StatusReply<Res>>) -> Req, {
    let response = target.ask_with_status(build);
    Self::install_timeout(target, &response, timeout);
    response
  }

  fn install_timeout<Req, Res>(target: &TypedActorRef<Req>, response: &TypedAskResponse<Res>, timeout: Duration)
  where
    Req: Send + Sync + 'static,
    Res: Send + Sync + 'static, {
    let future = response.future().clone().into_inner();
    if future.with_read(|inner| inner.is_ready()) {
      return;
    }
    if let Some(system) = target.as_untyped().system_state() {
      install_ask_timeout(&future, &system, timeout);
    } else {
      complete_with_timeout(&future);
    }
  }
}
