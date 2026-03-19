use alloc::{boxed::Box, collections::VecDeque, vec::Vec};
use core::{
  future::Future,
  marker::PhantomData,
  pin::Pin,
  task::{Context, Poll},
};

use super::super::{
  super::{DynValue, FlowLogic, StreamError, downcast_value},
  noop_waker,
};

pub(in crate::core::stage::flow) struct MapAsyncLogic<In, Out, F, Fut>
where
  Fut: Future<Output = Out> + Send + 'static, {
  pub(in crate::core::stage::flow) func:        F,
  pub(in crate::core::stage::flow) parallelism: usize,
  pub(in crate::core::stage::flow) pending:     VecDeque<MapAsyncEntry<Out, Fut>>,
  pub(in crate::core::stage::flow) _pd:         PhantomData<fn(In) -> Out>,
}

pub(in crate::core::stage::flow) enum MapAsyncEntry<Out, Fut>
where
  Fut: Future<Output = Out> + Send + 'static, {
  InFlight(Pin<Box<Fut>>),
  Completed(Out),
}

impl<Out, Fut> MapAsyncEntry<Out, Fut>
where
  Fut: Future<Output = Out> + Send + 'static,
{
  const fn is_pending(&self) -> bool {
    match self {
      | Self::InFlight(_) => true,
      | Self::Completed(_) => false,
    }
  }
}

impl<In, Out, F, Fut> FlowLogic for MapAsyncLogic<In, Out, F, Fut>
where
  In: Send + Sync + 'static,
  Out: Send + Sync + 'static,
  F: FnMut(In) -> Fut + Send + Sync + 'static,
  Fut: Future<Output = Out> + Send + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    let value = downcast_value::<In>(input)?;
    let future = (self.func)(value);
    self.pending.push_back(MapAsyncEntry::InFlight(Box::pin(future)));
    Ok(Vec::new())
  }

  fn can_accept_input(&self) -> bool {
    if self.parallelism == 0 {
      return false;
    }
    self.pending.iter().filter(|entry| entry.is_pending()).count() < self.parallelism
  }

  fn drain_pending(&mut self) -> Result<Vec<DynValue>, StreamError> {
    let waker = noop_waker();
    let mut cx = Context::from_waker(&waker);
    for entry in &mut self.pending {
      let MapAsyncEntry::InFlight(future) = entry else {
        continue;
      };
      if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
        *entry = MapAsyncEntry::Completed(output);
      }
    }

    let mut outputs = Vec::new();
    while let Some(entry) = self.pending.pop_front() {
      match entry {
        | MapAsyncEntry::Completed(output) => outputs.push(Box::new(output) as DynValue),
        | in_flight => {
          self.pending.push_front(in_flight);
          break;
        },
      }
    }
    Ok(outputs)
  }

  fn has_pending_output(&self) -> bool {
    !self.pending.is_empty()
  }

  fn on_restart(&mut self) -> Result<(), StreamError> {
    self.pending.clear();
    Ok(())
  }
}
