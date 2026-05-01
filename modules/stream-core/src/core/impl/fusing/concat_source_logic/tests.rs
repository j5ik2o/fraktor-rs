use super::SecondarySourceBridge;
use crate::core::dsl::Source;

#[test]
fn sync_terminal_state_returns_ok_when_completion_pending_and_stream_running() {
  // Construct a bridge over a finite source. Before driving the stream the
  // inner Sink::foreach completion future is still pending (`try_take` returns
  // `None`) and the stream itself is not yet terminal, exercising the
  // `is_terminal()` check branch in `sync_terminal_state`.
  let source = Source::from_array([1_u32, 2, 3]);
  let mut bridge = SecondarySourceBridge::new(source).expect("bridge");
  bridge.sync_terminal_state().expect("sync");
}
