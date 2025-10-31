#![cfg(feature = "std")]

extern crate alloc;

use core::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use cellactor_actor_core_rs::{
  ActorError, ActorRefSender, AnyMessage, DispatchExecutor, Dispatcher, InlineExecutor, Mailbox, MailboxMessage,
  MailboxPolicy, MessageInvoker, SystemMessage,
};
use cellactor_utils_core_rs::sync::ArcShared;

const MAILBOX_ITERATIONS: usize = 10_000;
const DISPATCHER_ITERATIONS: usize = 10_000;

#[derive(Debug)]
struct ThroughputReport {
  iterations: usize,
  elapsed:    Duration,
  per_second: f64,
}

#[test]
fn mailbox_throughput_harness_reports_positive_rate() {
  let report = measure_mailbox_throughput(MAILBOX_ITERATIONS);
  assert_eq!(report.iterations, MAILBOX_ITERATIONS);
  assert!(report.elapsed > Duration::ZERO);
  assert!(report.per_second.is_finite());
  assert!(report.per_second > 0.0);
}

#[test]
fn dispatcher_throughput_harness_processes_all_messages() {
  let report = measure_dispatcher_throughput(DISPATCHER_ITERATIONS);
  assert_eq!(report.iterations, DISPATCHER_ITERATIONS);
  assert!(report.elapsed > Duration::ZERO);
  assert!(report.per_second.is_finite());
  assert!(report.per_second > 0.0);
}

fn measure_mailbox_throughput(iterations: usize) -> ThroughputReport {
  let mailbox = Mailbox::new(MailboxPolicy::unbounded(None));

  let enqueue_start = Instant::now();
  for index in 0..iterations {
    mailbox.enqueue_user(AnyMessage::new(index as u32)).expect("enqueue");
  }
  let enqueue_duration = enqueue_start.elapsed();

  let dequeue_start = Instant::now();
  let mut processed = 0;
  while processed < iterations {
    match mailbox.dequeue() {
      | Some(MailboxMessage::User(_message)) => {
        processed += 1;
      },
      | other => panic!("expected user message, found {:?}", other),
    }
  }
  let dequeue_duration = dequeue_start.elapsed();

  let elapsed = enqueue_duration.saturating_add(dequeue_duration);
  throughput_report(iterations, elapsed)
}

fn measure_dispatcher_throughput(iterations: usize) -> ThroughputReport {
  let mailbox = ArcShared::new(Mailbox::new(MailboxPolicy::unbounded(None)));
  let executor: ArcShared<dyn DispatchExecutor> = ArcShared::new(InlineExecutor::new());
  let dispatcher = Dispatcher::new(mailbox, executor);
  let invoker = ArcShared::new(CountingInvoker::new());
  dispatcher.register_invoker(invoker.clone());

  let sender = dispatcher.into_sender();

  let start = Instant::now();
  for index in 0..iterations {
    sender.send(AnyMessage::new(index as u32)).expect("send");
  }
  let elapsed = start.elapsed();

  // Ensure any residual work is executed.
  dispatcher.schedule();

  assert_eq!(invoker.processed(), iterations);
  throughput_report(iterations, elapsed)
}

fn throughput_report(iterations: usize, elapsed: Duration) -> ThroughputReport {
  let seconds = elapsed.as_secs_f64().max(f64::EPSILON);
  let per_second = iterations as f64 / seconds;
  ThroughputReport { iterations, elapsed, per_second }
}

struct CountingInvoker {
  processed: AtomicUsize,
}

impl CountingInvoker {
  fn new() -> Self {
    Self { processed: AtomicUsize::new(0) }
  }

  fn processed(&self) -> usize {
    self.processed.load(Ordering::Relaxed)
  }
}

impl MessageInvoker for CountingInvoker {
  fn invoke_user_message(&self, message: AnyMessage) -> Result<(), ActorError> {
    drop(message);
    self.processed.fetch_add(1, Ordering::Relaxed);
    Ok(())
  }

  fn invoke_system_message(&self, message: SystemMessage) -> Result<(), ActorError> {
    drop(message);
    Ok(())
  }
}
