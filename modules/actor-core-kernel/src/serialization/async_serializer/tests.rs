use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
  any::Any,
  future::Future,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::{AsyncSerializer, SerializationFuture};
use crate::serialization::error::SerializationError;

/// Minimal async implementation for testing.
struct StubAsyncSerializer;

impl AsyncSerializer for StubAsyncSerializer {
  fn to_binary_async(&self, message: Box<dyn Any + Send + Sync>) -> SerializationFuture<'_, Vec<u8>> {
    Box::pin(async move {
      let s = message.downcast::<String>().map_err(|_| SerializationError::InvalidFormat)?;
      Ok(s.as_bytes().to_vec())
    })
  }

  fn from_binary_async(&self, bytes: Vec<u8>, _manifest: &str) -> SerializationFuture<'_, Box<dyn Any + Send + Sync>> {
    Box::pin(async move {
      let s = String::from_utf8(bytes).map_err(|_| SerializationError::InvalidFormat)?;
      Ok(Box::new(s) as Box<dyn Any + Send + Sync>)
    })
  }
}

/// Polls a future to completion synchronously.
///
/// Only works for futures that resolve immediately (no real I/O).
fn block_on<F: Future>(mut f: F) -> F::Output {
  // 安全性: この時点で future はピン留めされ、以降移動しない
  let mut f = unsafe { Pin::new_unchecked(&mut f) };

  fn noop_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(p: *const ()) -> RawWaker {
      RawWaker::new(p, &VTABLE)
    }
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(core::ptr::null(), &VTABLE)
  }

  let waker = unsafe { Waker::from_raw(noop_raw_waker()) };
  let mut cx = Context::from_waker(&waker);

  match f.as_mut().poll(&mut cx) {
    | Poll::Ready(val) => val,
    | Poll::Pending => panic!("future returned Pending in synchronous test"),
  }
}

#[test]
fn trait_object_is_send_sync() {
  fn assert_send_sync<T: Send + Sync + ?Sized>() {}
  assert_send_sync::<StubAsyncSerializer>();
  assert_send_sync::<dyn AsyncSerializer>();
}

#[test]
fn round_trip_async() {
  let serializer = StubAsyncSerializer;
  let message: Box<dyn Any + Send + Sync> = Box::new(String::from("async_hello"));

  let bytes = match block_on(serializer.to_binary_async(message)) {
    | Ok(v) => v,
    | Err(e) => panic!("to_binary_async failed: {e:?}"),
  };
  assert_eq!(bytes, b"async_hello");

  let restored = match block_on(serializer.from_binary_async(bytes, "")) {
    | Ok(v) => v,
    | Err(e) => panic!("from_binary_async failed: {e:?}"),
  };
  let restored_str = match restored.downcast_ref::<String>() {
    | Some(v) => v,
    | None => panic!("downcast to String failed"),
  };
  assert_eq!(restored_str, "async_hello");
}

#[test]
fn invalid_message_type_returns_error() {
  let serializer = StubAsyncSerializer;
  let message: Box<dyn Any + Send + Sync> = Box::new(42_i32);

  let result = block_on(serializer.to_binary_async(message));
  match result {
    | Err(err) => assert!(err.is_invalid_format()),
    | Ok(_) => panic!("expected InvalidFormat error"),
  }
}

#[test]
fn empty_bytes_produces_empty_string() {
  let serializer = StubAsyncSerializer;
  let bytes: Vec<u8> = Vec::new();

  let restored = match block_on(serializer.from_binary_async(bytes, "")) {
    | Ok(v) => v,
    | Err(e) => panic!("from_binary_async failed: {e:?}"),
  };
  let restored_str = match restored.downcast_ref::<String>() {
    | Some(v) => v,
    | None => panic!("downcast to String failed"),
  };
  assert_eq!(restored_str, "");
}

#[test]
fn serialization_future_type_alias_is_usable() {
  fn _accepts_future(_f: SerializationFuture<'_, Vec<u8>>) {}
}
