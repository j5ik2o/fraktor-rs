use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{
  future::Future,
  marker::PhantomData,
  pin::Pin,
  task::{Context, Poll},
};

use super::noop_waker;
use crate::core::{DynValue, FlowLogic, StreamError, downcast_value};

struct PendingEntry<In, P> {
  sequence:  u64,
  value:     In,
  partition: P,
}

enum InFlightEntry<Out, P, Fut>
where
  Fut: Future<Output = Out> + Send + 'static, {
  InFlight { sequence: u64, partition: P, future: Pin<Box<Fut>> },
  Completed { sequence: u64, partition: P, output: Out },
}

impl<Out, P, Fut> InFlightEntry<Out, P, Fut>
where
  Fut: Future<Output = Out> + Send + 'static,
{
  const fn sequence(&self) -> u64 {
    match self {
      | Self::InFlight { sequence, .. } | Self::Completed { sequence, .. } => *sequence,
    }
  }

  const fn partition(&self) -> &P {
    match self {
      | Self::InFlight { partition, .. } | Self::Completed { partition, .. } => partition,
    }
  }

  const fn is_in_flight(&self) -> bool {
    matches!(self, Self::InFlight { .. })
  }

  const fn is_completed(&self) -> bool {
    matches!(self, Self::Completed { .. })
  }
}

pub(in crate::core) struct MapAsyncPartitionedLogic<In, Out, P, Partitioner, F, Fut>
where
  Fut: Future<Output = Out> + Send + 'static, {
  partitioner:         Partitioner,
  func:                F,
  parallelism:         usize,
  ordered:             bool,
  waiting:             VecDeque<PendingEntry<In, P>>,
  in_flight:           Vec<InFlightEntry<Out, P, Fut>>,
  completed_unordered: VecDeque<Out>,
  next_sequence:       u64,
  next_emit:           u64,
  _pd:                 PhantomData<fn(In) -> Out>,
}

impl<In, Out, P, Partitioner, F, Fut> MapAsyncPartitionedLogic<In, Out, P, Partitioner, F, Fut>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  P: Clone + PartialEq + Send + Sync + 'static,
  Partitioner: FnMut(&In) -> P + Send + Sync + 'static,
  F: FnMut(In, P) -> Fut + Send + Sync + 'static,
  Fut: Future<Output = Out> + Send + 'static,
{
  pub(in crate::core) fn new(partitioner: Partitioner, func: F, parallelism: usize, ordered: bool) -> Self {
    Self {
      partitioner,
      func,
      parallelism,
      ordered,
      waiting: VecDeque::new(),
      in_flight: Vec::new(),
      completed_unordered: VecDeque::new(),
      next_sequence: 0,
      next_emit: 0,
      _pd: PhantomData,
    }
  }

  fn partition_is_busy(&self, partition: &P) -> bool {
    self.in_flight.iter().any(|entry| entry.is_in_flight() && entry.partition() == partition)
  }

  fn poll_in_flight(&mut self) {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    if self.ordered {
      for entry in &mut self.in_flight {
        let InFlightEntry::InFlight { sequence, partition, future } = entry else {
          continue;
        };
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
          *entry = InFlightEntry::Completed { sequence: *sequence, partition: partition.clone(), output };
        }
      }
      return;
    }
    let mut completed_indices = Vec::new();
    for (index, entry) in self.in_flight.iter_mut().enumerate() {
      let InFlightEntry::InFlight { future, .. } = entry else {
        continue;
      };
      if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
        self.completed_unordered.push_back(output);
        completed_indices.push(index);
      }
    }
    while let Some(index) = completed_indices.pop() {
      let _ = self.in_flight.remove(index);
    }
  }

  fn start_waiting(&mut self) {
    while self.in_flight.iter().filter(|entry| entry.is_in_flight()).count() < self.parallelism {
      let Some(index) = self.waiting.iter().position(|entry| !self.partition_is_busy(&entry.partition)) else {
        break;
      };
      let Some(PendingEntry { sequence, value, partition }) = self.waiting.remove(index) else {
        break;
      };
      let future = (self.func)(value, partition.clone());
      let entry = InFlightEntry::InFlight { sequence, partition, future: Box::pin(future) };
      if self.ordered {
        let insert_index = self.in_flight.partition_point(|existing| existing.sequence() < sequence);
        self.in_flight.insert(insert_index, entry);
      } else {
        self.in_flight.push(entry);
      }
    }
  }

  fn take_completed_by_sequence(&mut self, sequence: u64) -> Option<Out> {
    let index = self.in_flight.iter().position(
      |entry| matches!(entry, InFlightEntry::Completed { sequence: entry_sequence, .. } if *entry_sequence == sequence),
    )?;
    let InFlightEntry::Completed { output, .. } = self.in_flight.remove(index) else {
      return None;
    };
    Some(output)
  }

  fn take_any_completed(&mut self) -> Option<Out> {
    if let Some(output) = self.completed_unordered.pop_front() {
      return Some(output);
    }
    let index = self.in_flight.iter().position(InFlightEntry::is_completed)?;
    let InFlightEntry::Completed { output, .. } = self.in_flight.remove(index) else {
      return None;
    };
    Some(output)
  }
}

impl<In, Out, P, Partitioner, F, Fut> FlowLogic for MapAsyncPartitionedLogic<In, Out, P, Partitioner, F, Fut>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  P: Clone + PartialEq + Send + Sync + 'static,
  Partitioner: FnMut(&In) -> P + Send + Sync + 'static,
  F: FnMut(In, P) -> Fut + Send + Sync + 'static,
  Fut: Future<Output = Out> + Send + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let partition = (self.partitioner)(&value);
    let entry = PendingEntry { sequence: self.next_sequence, value, partition };
    self.next_sequence = self.next_sequence.saturating_add(1);
    self.waiting.push_back(entry);
    self.drain_pending()
  }

  fn can_accept_input(&self) -> bool {
    self.parallelism > 0 && self.waiting.len().saturating_add(self.in_flight.len()) < self.parallelism
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let mut outputs = Vec::new();

    loop {
      self.poll_in_flight();
      self.start_waiting();

      let next_output =
        if self.ordered { self.take_completed_by_sequence(self.next_emit) } else { self.take_any_completed() };

      let Some(output) = next_output else {
        break;
      };
      if self.ordered {
        self.next_emit = self.next_emit.saturating_add(1);
      }
      outputs.push(Box::new(output) as DynValue);
    }

    Ok(outputs)
  }

  fn has_pending_output(&self) -> bool {
    !self.waiting.is_empty() || !self.in_flight.is_empty() || !self.completed_unordered.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.waiting.clear();
    self.in_flight.clear();
    self.completed_unordered.clear();
    self.next_sequence = 0;
    self.next_emit = 0;
    Ok(())
  }
}
