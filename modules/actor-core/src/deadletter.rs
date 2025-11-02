//! Deadletter mailbox placeholder.

use core::marker::PhantomData;

use crate::{NoStdToolbox, RuntimeToolbox};

/// Collects undeliverable messages placeholder.
pub struct DeadletterGeneric<TB: RuntimeToolbox + 'static> {
  _marker: PhantomData<TB>,
}

impl<TB: RuntimeToolbox + 'static> Default for DeadletterGeneric<TB> {
  fn default() -> Self {
    Self { _marker: PhantomData }
  }
}

/// 既定ツールボックス向けの型エイリアス。
pub type Deadletter = DeadletterGeneric<NoStdToolbox>;
