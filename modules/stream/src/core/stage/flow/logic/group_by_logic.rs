use alloc::{boxed::Box, vec, vec::Vec};
use core::marker::PhantomData;

use super::super::super::{DynValue, FlowLogic, StreamError, downcast_value};

#[cfg(test)]
mod tests;

pub(in crate::core::stage::flow) struct GroupByLogic<In, Key, F> {
  pub(in crate::core::stage::flow) max_substreams: usize,
  pub(in crate::core::stage::flow) seen_keys:      Vec<Key>,
  pub(in crate::core::stage::flow) key_fn:         F,
  pub(in crate::core::stage::flow) _pd:            PhantomData<fn(In) -> Key>,
}

impl<In, Key, F> FlowLogic for GroupByLogic<In, Key, F>
where
  In: Send + Sync + 'static,
  Key: Clone + PartialEq + Send + Sync + 'static,
  F: FnMut(&In) -> Key + Send + Sync + 'static,
{
  fn apply(&mut self, input: DynValue) -> Result<Vec<DynValue>, StreamError> {
    if self.max_substreams == 0 {
      return Err(StreamError::InvalidConnection);
    }
    let value = downcast_value::<In>(input)?;
    let key = (self.key_fn)(&value);
    if !self.seen_keys.contains(&key) {
      if self.seen_keys.len() >= self.max_substreams {
        return Err(StreamError::SubstreamLimitExceeded { max_substreams: self.max_substreams });
      }
      self.seen_keys.push(key.clone());
    }
    Ok(vec![Box::new((key, value)) as DynValue])
  }
}
