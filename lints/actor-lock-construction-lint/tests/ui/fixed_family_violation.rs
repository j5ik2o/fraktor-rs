#![feature(register_tool)]
#![register_tool(actor_lock_construction)]
#![warn(actor_lock_construction::actor_lock_construction)]

struct SpinSyncMutex<T>(T);

impl<T> SpinSyncMutex<T> {
  fn new(value: T) -> Self {
    Self(value)
  }
}

fn build_lock() -> SpinSyncMutex<u32> {
  SpinSyncMutex::new(1)
}

fn main() {}
