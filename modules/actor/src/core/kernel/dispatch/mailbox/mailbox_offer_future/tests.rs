use alloc::format;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
  time::Duration,
};

use fraktor_utils_rs::core::{
  sync::{ArcShared, NoStdMutex},
  timing::delay::ManualDelayProvider,
};

use crate::core::kernel::{
  actor::{Pid, error::SendError, messaging::AnyMessage},
  dispatch::mailbox::{EnqueueOutcome, Mailbox, MailboxOverflowStrategy, MailboxPolicy},
  event::stream::{EventStreamEvent, EventStreamSubscriber, subscriber_handle},
  system::ActorSystem,
};

unsafe fn noop_clone(_: *const ()) -> RawWaker {
  noop_raw_waker()
}

unsafe fn noop_wake(_: *const ()) {}

unsafe fn noop_wake_by_ref(_: *const ()) {}

unsafe fn noop_drop(_: *const ()) {}

const NOOP_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop_wake, noop_wake_by_ref, noop_drop);

fn noop_raw_waker() -> RawWaker {
  RawWaker::new(core::ptr::null(), &NOOP_WAKER_VTABLE)
}

fn noop_waker() -> Waker {
  unsafe { Waker::from_raw(noop_raw_waker()) }
}

#[test]
fn mailbox_offer_future_bounded_block_completes_when_space_available() {
  use core::num::NonZeroUsize;

  let mailbox =
    Mailbox::new(MailboxPolicy::bounded(NonZeroUsize::new(1).unwrap(), MailboxOverflowStrategy::Block, None));

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(0)), Ok(EnqueueOutcome::Enqueued)));

  let mut future = match mailbox.enqueue_user(AnyMessage::new(1)) {
    | Ok(EnqueueOutcome::Pending(future)) => future,
    | Ok(EnqueueOutcome::Enqueued) => panic!("expected pending offer future"),
    | Err(error) => panic!("unexpected enqueue error: {error:?}"),
  };

  let _ = mailbox.dequeue();

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  let result = Pin::new(&mut future).poll(&mut context);
  assert!(matches!(result, Poll::Ready(Ok(()))));
}

#[test]
fn mailbox_offer_future_debug_format() {
  use core::num::NonZeroUsize;

  let mailbox =
    Mailbox::new(MailboxPolicy::bounded(NonZeroUsize::new(1).unwrap(), MailboxOverflowStrategy::Block, None));

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(0)), Ok(EnqueueOutcome::Enqueued)));
  let future = match mailbox.enqueue_user(AnyMessage::new(1)) {
    | Ok(EnqueueOutcome::Pending(future)) => future,
    | Ok(EnqueueOutcome::Enqueued) => panic!("expected pending offer future"),
    | Err(error) => panic!("unexpected enqueue error: {error:?}"),
  };

  let debug_str = format!("{:?}", future);
  assert!(debug_str.contains("MailboxOfferFuture"));
}

#[test]
fn mailbox_offer_future_times_out_and_returns_send_error() {
  use core::num::NonZeroUsize;

  let mailbox =
    Mailbox::new(MailboxPolicy::bounded(NonZeroUsize::new(1).unwrap(), MailboxOverflowStrategy::Block, None));

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(0)), Ok(EnqueueOutcome::Enqueued)));

  let mut provider = ManualDelayProvider::new();
  let mut future = match mailbox.enqueue_user(AnyMessage::new(1)) {
    | Ok(EnqueueOutcome::Pending(future)) => future.with_timeout(Duration::from_millis(5), &mut provider),
    | Ok(EnqueueOutcome::Enqueued) => panic!("expected pending offer future"),
    | Err(error) => panic!("unexpected enqueue error: {error:?}"),
  };

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);

  assert!(matches!(Pin::new(&mut future).poll(&mut context), Poll::Pending));
  assert!(provider.trigger_next());
  let result = Pin::new(&mut future).poll(&mut context);
  assert!(matches!(result, Poll::Ready(Err(SendError::Timeout(_)))));
}

#[test]
fn mailbox_offer_future_republishes_metrics_after_pending_offer_completes() {
  use alloc::vec::Vec;
  use core::num::NonZeroUsize;

  use crate::core::kernel::dispatch::mailbox::MailboxInstrumentation;

  let mailbox =
    Mailbox::new(MailboxPolicy::bounded(NonZeroUsize::new(1).unwrap(), MailboxOverflowStrategy::Block, None));
  let system_state = ActorSystem::new_empty().state();
  let pid = Pid::new(9, 0);
  let instrumentation = MailboxInstrumentation::new(system_state.clone(), pid, Some(4), None, None);
  mailbox.set_instrumentation(instrumentation);

  let events = ArcShared::new(NoStdMutex::new(Vec::new()));
  let subscriber = subscriber_handle(TestSubscriber::new(events.clone()));
  let _subscription = system_state.event_stream().subscribe(&subscriber);

  assert!(matches!(mailbox.enqueue_user(AnyMessage::new(0)), Ok(EnqueueOutcome::Enqueued)));
  let mut future = match mailbox.enqueue_user(AnyMessage::new(1)) {
    | Ok(EnqueueOutcome::Pending(future)) => future,
    | Ok(EnqueueOutcome::Enqueued) => panic!("expected pending offer future"),
    | Err(error) => panic!("unexpected enqueue error: {error:?}"),
  };

  let _ = mailbox.dequeue();

  let waker = noop_waker();
  let mut context = Context::from_waker(&waker);
  let result = Pin::new(&mut future).poll(&mut context);
  assert!(matches!(result, Poll::Ready(Ok(()))));

  let guard = events.lock();
  let latest = guard.iter().rev().find_map(|event| match event {
    | EventStreamEvent::Mailbox(event) if event.pid() == pid => Some((event.user_len(), event.system_len())),
    | _ => None,
  });
  assert_eq!(latest, Some((1, 0)));
}

struct TestSubscriber {
  events: ArcShared<NoStdMutex<alloc::vec::Vec<EventStreamEvent>>>,
}

impl TestSubscriber {
  fn new(events: ArcShared<NoStdMutex<alloc::vec::Vec<EventStreamEvent>>>) -> Self {
    Self { events }
  }
}

impl EventStreamSubscriber for TestSubscriber {
  fn on_event(&mut self, event: &EventStreamEvent) {
    self.events.lock().push(event.clone());
  }
}
