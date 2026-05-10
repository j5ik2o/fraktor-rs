use core::num::NonZeroUsize;
use std::hint::black_box;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use fraktor_actor_core_kernel_rs::{
  actor::messaging::AnyMessage,
  dispatch::mailbox::{
    BoundedMessageQueue, Envelope, Mailbox, MailboxOverflowStrategy, MailboxPolicy, MessagePriorityGenerator,
    MessageQueue, UnboundedControlAwareMessageQueue, UnboundedMessageQueue, UnboundedPriorityMessageQueue,
    UnboundedPriorityMessageQueueState, UnboundedPriorityMessageQueueStateShared,
  },
};
use fraktor_utils_core_rs::sync::ArcShared;

const BATCH_SIZES: [usize; 3] = [1, 64, 1024];

struct PayloadPriorityGenerator;

impl MessagePriorityGenerator for PayloadPriorityGenerator {
  fn priority(&self, message: &AnyMessage) -> i32 {
    message.payload().downcast_ref::<i32>().copied().unwrap_or(i32::MAX)
  }
}

fn capacity(value: usize) -> NonZeroUsize {
  NonZeroUsize::new(value).expect("benchmark capacity must be non-zero")
}

fn unbounded_queue() -> UnboundedMessageQueue {
  UnboundedMessageQueue::new()
}

fn bounded_queue(size: usize) -> BoundedMessageQueue {
  BoundedMessageQueue::new(capacity(size), MailboxOverflowStrategy::DropNewest)
}

fn control_aware_queue() -> UnboundedControlAwareMessageQueue {
  UnboundedControlAwareMessageQueue::new()
}

fn priority_queue() -> UnboundedPriorityMessageQueue {
  let generator: ArcShared<dyn MessagePriorityGenerator> = ArcShared::new(PayloadPriorityGenerator);
  let state_shared = UnboundedPriorityMessageQueueStateShared::new(UnboundedPriorityMessageQueueState::new());
  UnboundedPriorityMessageQueue::new(generator, state_shared)
}

fn enqueue_normal_batch(queue: &dyn MessageQueue, batch_size: usize) {
  for sequence in 0..batch_size {
    queue.enqueue(Envelope::new(AnyMessage::new(sequence as u64))).expect("enqueue benchmark message");
  }
}

fn enqueue_control_aware_batch(queue: &dyn MessageQueue, batch_size: usize) {
  for sequence in 0..batch_size {
    let message =
      if sequence % 4 == 0 { AnyMessage::control(sequence as u64) } else { AnyMessage::new(sequence as u64) };
    queue.enqueue(Envelope::new(message)).expect("enqueue benchmark message");
  }
}

fn enqueue_priority_batch(queue: &dyn MessageQueue, batch_size: usize) {
  for sequence in 0..batch_size {
    queue.enqueue(Envelope::new(AnyMessage::new((batch_size - sequence) as i32))).expect("enqueue benchmark message");
  }
}

fn drain_queue(queue: &dyn MessageQueue) {
  while let Some(envelope) = queue.dequeue() {
    black_box(envelope);
  }
}

fn bench_message_queue_enqueue(c: &mut Criterion) {
  let mut group = c.benchmark_group("message_queue_enqueue");

  for batch_size in BATCH_SIZES {
    group.throughput(Throughput::Elements(batch_size as u64));

    group.bench_with_input(format!("unbounded_batch_{batch_size}"), &batch_size, |b, &batch_size| {
      b.iter_batched(
        unbounded_queue,
        |queue| {
          enqueue_normal_batch(&queue, batch_size);
          black_box(queue);
        },
        BatchSize::SmallInput,
      );
    });

    group.bench_with_input(format!("bounded_batch_{batch_size}"), &batch_size, |b, &batch_size| {
      b.iter_batched(
        || bounded_queue(batch_size),
        |queue| {
          enqueue_normal_batch(&queue, batch_size);
          black_box(queue);
        },
        BatchSize::SmallInput,
      );
    });

    group.bench_with_input(format!("control_aware_batch_{batch_size}"), &batch_size, |b, &batch_size| {
      b.iter_batched(
        control_aware_queue,
        |queue| {
          enqueue_control_aware_batch(&queue, batch_size);
          black_box(queue);
        },
        BatchSize::SmallInput,
      );
    });

    group.bench_with_input(format!("priority_batch_{batch_size}"), &batch_size, |b, &batch_size| {
      b.iter_batched(
        priority_queue,
        |queue| {
          enqueue_priority_batch(&queue, batch_size);
          black_box(queue);
        },
        BatchSize::SmallInput,
      );
    });
  }

  group.finish();
}

fn bench_message_queue_drain(c: &mut Criterion) {
  let mut group = c.benchmark_group("message_queue_drain");

  for batch_size in BATCH_SIZES {
    group.throughput(Throughput::Elements(batch_size as u64));

    group.bench_with_input(format!("unbounded_batch_{batch_size}"), &batch_size, |b, &batch_size| {
      b.iter_batched(
        || {
          let queue = unbounded_queue();
          enqueue_normal_batch(&queue, batch_size);
          queue
        },
        |queue| drain_queue(&queue),
        BatchSize::SmallInput,
      );
    });

    group.bench_with_input(format!("bounded_batch_{batch_size}"), &batch_size, |b, &batch_size| {
      b.iter_batched(
        || {
          let queue = bounded_queue(batch_size);
          enqueue_normal_batch(&queue, batch_size);
          queue
        },
        |queue| drain_queue(&queue),
        BatchSize::SmallInput,
      );
    });

    group.bench_with_input(format!("control_aware_batch_{batch_size}"), &batch_size, |b, &batch_size| {
      b.iter_batched(
        || {
          let queue = control_aware_queue();
          enqueue_control_aware_batch(&queue, batch_size);
          queue
        },
        |queue| drain_queue(&queue),
        BatchSize::SmallInput,
      );
    });

    group.bench_with_input(format!("priority_batch_{batch_size}"), &batch_size, |b, &batch_size| {
      b.iter_batched(
        || {
          let queue = priority_queue();
          enqueue_priority_batch(&queue, batch_size);
          queue
        },
        |queue| drain_queue(&queue),
        BatchSize::SmallInput,
      );
    });
  }

  group.finish();
}

fn bench_mailbox_enqueue(c: &mut Criterion) {
  let mut group = c.benchmark_group("mailbox_enqueue");

  for batch_size in BATCH_SIZES {
    group.throughput(Throughput::Elements(batch_size as u64));

    group.bench_with_input(format!("unbounded_batch_{batch_size}"), &batch_size, |b, &batch_size| {
      b.iter_batched(
        || Mailbox::new(MailboxPolicy::unbounded(None)),
        |mailbox| {
          for sequence in 0..batch_size {
            mailbox.enqueue_user(AnyMessage::new(sequence as u64)).expect("enqueue benchmark message");
          }
          black_box(mailbox);
        },
        BatchSize::SmallInput,
      );
    });

    group.bench_with_input(format!("bounded_batch_{batch_size}"), &batch_size, |b, &batch_size| {
      b.iter_batched(
        || Mailbox::new(MailboxPolicy::bounded(capacity(batch_size), MailboxOverflowStrategy::DropNewest, None)),
        |mailbox| {
          for sequence in 0..batch_size {
            mailbox.enqueue_user(AnyMessage::new(sequence as u64)).expect("enqueue benchmark message");
          }
          black_box(mailbox);
        },
        BatchSize::SmallInput,
      );
    });
  }

  group.finish();
}

fn bench_mailbox_overflow(c: &mut Criterion) {
  let mut group = c.benchmark_group("mailbox_overflow");

  for overflow in [MailboxOverflowStrategy::DropNewest, MailboxOverflowStrategy::DropOldest] {
    let overflow_name = match overflow {
      | MailboxOverflowStrategy::DropNewest => "drop_newest",
      | MailboxOverflowStrategy::DropOldest => "drop_oldest",
      | MailboxOverflowStrategy::Grow => "grow",
    };

    group.bench_function(overflow_name, |b| {
      b.iter_batched(
        || Mailbox::new(MailboxPolicy::bounded(capacity(64), overflow, None)),
        |mailbox| {
          for sequence in 0..128 {
            mailbox.enqueue_user(AnyMessage::new(sequence as u64)).expect("enqueue benchmark message");
          }
          black_box(mailbox);
        },
        BatchSize::SmallInput,
      );
    });
  }

  group.finish();
}

criterion_group!(
  mailbox,
  bench_message_queue_enqueue,
  bench_message_queue_drain,
  bench_mailbox_enqueue,
  bench_mailbox_overflow
);
criterion_main!(mailbox);
