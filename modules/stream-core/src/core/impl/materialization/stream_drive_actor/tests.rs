extern crate std;

use super::super::{Stream, StreamDriveActor, StreamShared, StreamState};
use crate::core::{
  dsl::{Sink, Source},
  r#impl::fusing::StreamBufferConfig,
  materialization::KeepRight,
};

fn running_stream() -> StreamShared {
  let graph = Source::single(1_u32).into_mat(Sink::head(), KeepRight);
  let (plan, _completion) = graph.into_parts();
  let mut stream = Stream::new(plan, StreamBufferConfig::default());
  stream.start().expect("stream should start");
  StreamShared::new(stream)
}

#[test]
fn register_stores_stream_by_stream_id() {
  let mut actor = StreamDriveActor::new();
  let stream = running_stream();
  let id = stream.id();

  actor.register(stream).expect("register");

  assert!(actor.streams.contains_key(&id));
}

#[test]
fn tick_removes_terminal_streams() {
  let mut actor = StreamDriveActor::new();
  let stream = running_stream();
  let id = stream.id();
  actor.register(stream).expect("register");

  for _ in 0..16 {
    actor.tick();
    if !actor.streams.contains_key(&id) {
      break;
    }
  }

  assert!(!actor.streams.contains_key(&id));
}

#[test]
fn shutdown_clears_registered_streams_and_marks_shutdown_requested() {
  let mut actor = StreamDriveActor::new();
  actor.register(running_stream()).expect("register");

  actor.shutdown().expect("shutdown");

  assert!(actor.streams.is_empty());
  assert!(actor.shutdown_requested);
}

#[test]
fn register_after_shutdown_cancels_stream_without_storing_it() {
  let mut actor = StreamDriveActor::new();
  actor.shutdown().expect("shutdown");
  let stream = running_stream();

  actor.register(stream.clone()).expect("register after shutdown");

  assert!(actor.streams.is_empty());
  assert_eq!(stream.state(), StreamState::Cancelled);
}
