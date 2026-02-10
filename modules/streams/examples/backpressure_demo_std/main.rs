//! Backpressure-oriented buffer demo with bounded capacity and overflow handling.

use fraktor_streams_rs::core::{StreamBuffer, StreamBufferConfig, StreamError};
use fraktor_utils_rs::core::collections::queue::OverflowPolicy;

fn main() {
  let config = StreamBufferConfig::new(1, OverflowPolicy::Block);
  let buffer = StreamBuffer::new(config);

  let first = buffer.offer(10_u32);
  let second = buffer.offer(11_u32);

  println!("first offer: {first:?}");
  match second {
    | Ok(outcome) => println!("second offer unexpectedly succeeded: {outcome:?}"),
    | Err(StreamError::BufferOverflow) => println!("second offer hit backpressure: stream buffer overflow"),
    | Err(error) => println!("second offer failed with unexpected error: {error}"),
  }

  let drained = buffer.poll().expect("poll first item");
  println!("drained item after backpressure: {drained}");

  let retry = buffer.offer(11_u32).expect("offer after drain");
  println!("retry offer after drain: {retry:?}");
}
